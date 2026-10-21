"""Noten-Modus: gesungene Melodie erkennen und mit echtem Klavier nachspielen.

Datei → [Instrumental entfernen] → [Hall/Echo entfernen] → [Rauschen reduzieren]
      → Tonhöhe (RMVPE) → Noten (Tonhöhe, Anschlag, Länge) → MIDI → Salamander-Klavier (FluidSynth)
      → [Begleitung wieder dazumischen] → Ausgabe

Anders als RVC erzeugt das hier keinen neuen Klang, sondern spielt echte Klavier-Samples:
klingt sicher nach Klavier, verliert aber Gleittöne und Vibrato der Stimme.
"""

import shutil
import subprocess
import tempfile
from pathlib import Path

from . import protocol
from .convert import (APPLIO_DIR, DENOISE_MODEL, DEREVERB_MODEL, ENGINE_DIR, SILENCE_DB, VOCALS_MODEL,
                      _mix, _peak_db, _separate, _to_wav)

REPO_DIR = ENGINE_DIR.parent
FLUIDSYNTH = ENGINE_DIR / ".tools" / "fluidsynth" / "fluidsynth-v2.6.1-win10-x64-cpp11" / "bin" / "fluidsynth.exe"
PIANO_SF2 = (REPO_DIR / "models" / "instruments" / "salamander" / "SalamanderGrandPiano-SF2-V3+20200602"
             / "SalamanderGrandPiano-V3+20200602.sf2")

FRAME_S = 0.01         # RMVPE liefert 100 Tonhöhen pro Sekunde
MIN_NOTE_S = 0.12      # kürzere Schnipsel gehören zur Nachbarnote
SWITCH_SEMITONES = 0.8  # so weit muss die Stimme abweichen …
SWITCH_FRAMES = 4       # … und so lange (40 ms), damit eine neue Note beginnt
ONSET_DB = 6.0         # Lautstärkesprung, der einen neuen Anschlag auf gleicher Höhe auslöst
QUIET_DB = 45.0        # leiser als (lautester Moment − 45 dB) gilt als Pause


def _pitch_track(wav: Path):
    """Tonhöhe (MIDI-Wert, NaN = stumm) und Lautstärke (dB) je 10 ms."""
    import numpy as np
    import soundfile as sf
    import torch

    audio, sr = sf.read(str(wav), dtype="float32")
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    if sr != 16000:
        import librosa
        audio = librosa.resample(audio, orig_sr=sr, target_sr=16000)

    import sys
    if str(APPLIO_DIR) not in sys.path:
        sys.path.insert(0, str(APPLIO_DIR))
    from rvc.lib.predictors.RMVPE import RMVPE0Predictor  # noqa: PLC0415

    device = "cuda" if torch.cuda.is_available() else "cpu"
    rmvpe = RMVPE0Predictor(str(APPLIO_DIR / "rvc" / "models" / "predictors" / "rmvpe.pt"), device=device)
    f0 = rmvpe.infer_from_audio(audio, thred=0.03)

    hop = 160
    frames = len(f0)
    padded = np.pad(audio, (0, max(0, frames * hop - len(audio))))[: frames * hop]
    rms = np.sqrt(np.mean(padded.reshape(frames, hop) ** 2, axis=1))
    db = 20 * np.log10(rms + 1e-9)

    midi = np.full(frames, np.nan)
    voiced = (f0 > 0) & (db > db.max() - QUIET_DB)
    midi[voiced] = 69 + 12 * np.log2(f0[voiced] / 440.0)
    return midi, db


def _smooth(midi):
    """Medianfilter über 5 Frames, nur innerhalb gesungener Abschnitte."""
    import numpy as np

    out = midi.copy()
    for i in range(len(midi)):
        if np.isnan(midi[i]):
            continue
        window = midi[max(0, i - 2): i + 3]
        out[i] = np.nanmedian(window)
    return out


def _tuning(midi) -> float:
    """Wie weit die Stimme insgesamt neben dem Klavier liegt (−0,5 … 0,5 Halbtöne)."""
    import numpy as np

    cents = midi[~np.isnan(midi)]
    if cents.size == 0:
        return 0.0
    angle = 2 * np.pi * (cents - np.round(cents))
    return float(np.angle(np.mean(np.exp(1j * angle))) / (2 * np.pi))


def find_notes(midi, db, snap_to_key: bool = True) -> list[tuple[float, float, int, int]]:
    """Zerlegt die Tonhöhenkurve in Noten: (Start s, Ende s, MIDI-Note, Anschlag 1–127)."""
    import numpy as np

    midi = _smooth(midi) - _tuning(midi)
    segments: list[list[int]] = []  # [start, ende) in Frames
    start, ref, off = None, None, 0
    for i, m in enumerate(midi):
        if np.isnan(m):
            if start is not None:
                segments.append([start, i])
                start = None
            continue
        onset = i >= 3 and db[i] - db[i - 3] > ONSET_DB and start is not None and i - start > SWITCH_FRAMES
        if start is None:
            start, ref, off = i, m, 0
        elif onset:
            segments.append([start, i])
            start, ref, off = i, m, 0
        elif abs(m - ref) > SWITCH_SEMITONES:
            off += 1
            if off >= SWITCH_FRAMES:
                cut = i - off + 1
                segments.append([start, cut])
                start, ref, off = cut, m, 0
        else:
            off = 0
            ref = float(np.median(midi[start: i + 1]))
    if start is not None:
        segments.append([start, len(midi)])

    merged = _absorb_short(segments, midi)
    pitches = [float(np.nanmedian(midi[a:b])) for a, b in merged]
    if snap_to_key and merged:
        scale = _key_scale(pitches, [b - a for a, b in merged])
        pitches = [_snap(p, scale) for p in pitches]

    loudest = max((db[a:b].max() for a, b in merged), default=0.0)
    notes = []
    for (a, b), pitch in zip(merged, pitches):
        level = db[a:b].max() - (loudest - 30)  # 0 … 30 dB unter dem Lautesten
        velocity = int(np.clip(40 + level / 30 * 80, 25, 120))
        notes.append((a * FRAME_S, b * FRAME_S, int(round(pitch)), velocity))
    return notes


def _absorb_short(segments: list[list[int]], midi) -> list[list[int]]:
    """Kurze Schnipsel (Gleittöne, Übergänge) dem angrenzenden Ton mit der näheren Höhe zuschlagen."""
    import numpy as np

    min_frames = round(MIN_NOTE_S / FRAME_S)
    segs = [list(s) for s in segments]
    while True:
        short = [i for i, (a, b) in enumerate(segs) if b - a < min_frames]
        if not short:
            return segs
        i = min(short, key=lambda k: segs[k][1] - segs[k][0])
        a, b = segs[i]
        here = np.nanmedian(midi[a:b])
        options = []
        if i > 0 and segs[i - 1][1] == a:
            options.append((abs(np.nanmedian(midi[segs[i - 1][0]:segs[i - 1][1]]) - here), i - 1))
        if i + 1 < len(segs) and segs[i + 1][0] == b:
            options.append((abs(np.nanmedian(midi[segs[i + 1][0]:segs[i + 1][1]]) - here), i + 1))
        if options:
            j = min(options)[1]
            segs[j] = [min(segs[j][0], a), max(segs[j][1], b)]
        del segs[i]


MAJOR = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88]  # Krumhansl-Kessler
MINOR = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17]
STEPS = {"dur": [0, 2, 4, 5, 7, 9, 11], "moll": [0, 2, 3, 5, 7, 8, 10, 11]}  # Moll mit Leitton


def _key_scale(pitches: list[float], weights: list[int]) -> set[int]:
    """Tonart aus der Tonhöhenverteilung schätzen; gibt die erlaubten Tonklassen zurück."""
    import numpy as np

    hist = np.zeros(12)
    for p, w in zip(pitches, weights):
        hist[int(round(p)) % 12] += w
    best = (-2.0, 0, "dur")
    for root in range(12):
        for mode, profile in (("dur", MAJOR), ("moll", MINOR)):
            r = np.corrcoef(hist, np.roll(profile, root))[0, 1]
            if r > best[0]:
                best = (r, root, mode)
    _, root, mode = best
    return {(root + s) % 12 for s in STEPS[mode]}


def _snap(pitch: float, scale: set[int]) -> float:
    """Auf den nächsten Ton der Tonart ziehen (bei Gleichstand auf den gerundeten Ton)."""
    candidates = [n for n in range(int(pitch) - 2, int(pitch) + 3) if n % 12 in scale]
    return min(candidates, key=lambda n: (abs(n - pitch), n != round(pitch)))


def _write_midi(notes, transpose: int, path: Path) -> None:
    import mido

    tpb, tempo = 480, 500000  # 120 BPM: 1 s = 960 Ticks
    ticks = lambda seconds: round(seconds * tpb * 1_000_000 / tempo)  # noqa: E731
    mid = mido.MidiFile(ticks_per_beat=tpb)
    track = mido.MidiTrack()
    mid.tracks.append(track)
    track.append(mido.MetaMessage("set_tempo", tempo=tempo))
    events = []
    for start, end, pitch, velocity in notes:
        note = min(108, max(21, pitch + transpose))
        events.append((ticks(start), 1, note, velocity))
        events.append((ticks(end), 0, note, 0))
    now = 0
    for tick, on, note, velocity in sorted(events):
        kind = "note_on" if on else "note_off"
        track.append(mido.Message(kind, note=note, velocity=velocity, time=tick - now))
        now = tick
    mid.save(str(path))


def _render(midi_path: Path, target: Path) -> None:
    if not FLUIDSYNTH.exists() or not PIANO_SF2.exists():
        raise FileNotFoundError("Klavier fehlt – bitte tools\\setup-piano.ps1 ausführen")
    subprocess.run(
        [str(FLUIDSYNTH), "-ni", "-q", "-g", "1.0", "-r", "48000",
         "-o", "synth.reverb.active=0", "-o", "synth.chorus.active=0",
         "-F", str(target), str(PIANO_SF2), str(midi_path)],
        check=True, stdout=subprocess.DEVNULL,
    )


def notes_mode(args) -> None:
    source = Path(args.input).resolve()
    out = Path(args.out).resolve()
    if not source.exists():
        raise FileNotFoundError(f"Eingabe fehlt: {source}")
    if args.mix_instrumental and not args.isolate_vocals:
        raise ValueError("--mix-instrumental geht nur zusammen mit --isolate-vocals")
    if not -24 <= args.transpose <= 24:
        raise ValueError("--transpose muss zwischen -24 und 24 liegen")
    out.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="tunify_") as tmp:
        work = Path(tmp)
        current = work / "input.wav"
        protocol.progress("load", 2, "Datei einlesen")
        _to_wav(source, current)

        instrumental = None
        if args.isolate_vocals:
            protocol.progress("isolate", 10, "Instrumental entfernen")
            current, instrumental = _separate(VOCALS_MODEL, current, work, keep="Vocals")
        if args.dereverb:
            protocol.progress("dereverb", 35, "Hall/Echo entfernen")
            current, _ = _separate(DEREVERB_MODEL, current, work, keep="No Reverb")
        if args.denoise:
            protocol.progress("denoise", 50, "Rauschen reduzieren")
            current, _ = _separate(DENOISE_MODEL, current, work, keep="No Noise")
        if _peak_db(current) < SILENCE_DB:
            raise ValueError("Keine Stimme gefunden: Die Spur ist nach dem Vorverarbeiten still.")

        protocol.progress("pitch", 60, "Töne erkennen")
        midi, db = _pitch_track(current)
        notes = find_notes(midi, db, snap_to_key=not args.chromatic)
        if not notes:
            raise ValueError("Keine Töne erkannt. Ist in der Aufnahme Gesang?")

        protocol.progress("render", 80, f"{len(notes)} Töne auf dem Klavier spielen")
        midi_path = work / "melodie.mid"
        _write_midi(notes, args.transpose, midi_path)
        if args.midi:
            Path(args.midi).resolve().parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(midi_path, Path(args.midi).resolve())
        rendered = work / "klavier.wav"
        _render(midi_path, rendered)

        if args.mix_instrumental and instrumental is not None:
            protocol.progress("mix", 90, "Begleitung dazumischen")
            mixed = work / "mixed.wav"
            _mix(rendered, instrumental, mixed)
            rendered = mixed

        protocol.progress("save", 97, "Speichern")
        if out.suffix.lower() == ".wav":
            shutil.copyfile(rendered, out)
        else:
            subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(rendered), str(out)], check=True)

    protocol.progress("done", 100, "Fertig")
    protocol.emit("result", output=str(out), notes=len(notes))
