"""Stimme umwandeln: der Ablauf von Musicfys „Convert Voice“, lokal.

Datei → [Instrumental entfernen] → [Hall/Echo entfernen] → [Rauschen reduzieren]
      → RVC (Applio) mit Pitch und Formant → [Begleitung wieder dazumischen] → Ausgabe
"""

import os
import shutil
import sys
import tempfile
from pathlib import Path

from . import proc, protocol

ENGINE_DIR = Path(__file__).resolve().parent.parent
APPLIO_DIR = ENGINE_DIR / "vendor" / "applio"
SEPARATOR_MODELS_DIR = ENGINE_DIR.parent / "models" / "separator"

# UVR-Modelle für python-audio-separator (werden beim ersten Gebrauch geladen)
VOCALS_MODEL = "model_bs_roformer_ep_317_sdr_12.9755.ckpt"
DEREVERB_MODEL = "UVR-DeEcho-DeReverb.pth"
DENOISE_MODEL = "UVR-DeNoise.pth"


def _applio():
    """Applio erwartet, dass es aus seinem eigenen Ordner läuft."""
    os.chdir(APPLIO_DIR)
    if str(APPLIO_DIR) not in sys.path:
        sys.path.insert(0, str(APPLIO_DIR))
    import core  # noqa: PLC0415 – erst nach chdir importierbar

    return core


def prepare() -> None:
    protocol.progress("prepare", 0, "Grundmodelle laden (RMVPE, ContentVec)")
    core = _applio()
    core.run_prerequisites_script(pretraineds_hifigan=False, models=True, exe=False)
    protocol.progress("prepare", 100, "Grundmodelle bereit")
    protocol.emit("result", applio=str(APPLIO_DIR))


def _separate(model: str, source: Path, work: Path, keep: str) -> tuple[Path, Path | None]:
    """Trennt `source` mit einem UVR-Modell. Gibt (behaltene Spur, andere Spur) zurück.

    `keep` ist der Spurname in Klammern, z. B. "Vocals", "No Reverb" oder "No Noise".
    """
    from audio_separator.separator import Separator

    SEPARATOR_MODELS_DIR.mkdir(parents=True, exist_ok=True)
    out_dir = work / Path(model).stem
    out_dir.mkdir(parents=True, exist_ok=True)
    separator = Separator(output_dir=str(out_dir), model_file_dir=str(SEPARATOR_MODELS_DIR), output_format="WAV")
    separator.load_model(model_filename=model)
    files = [out_dir / Path(f).name for f in separator.separate(str(source))]
    kept = [f for f in files if f"({keep})".lower() in f.name.lower()]
    if not kept:
        raise RuntimeError(f"Spur „{keep}“ nicht gefunden in: {[f.name for f in files]}")
    others = [f for f in files if f not in kept]
    return kept[0], (others[0] if others else None)


SILENCE_DB = -60.0


def _isolate_message(text: str, wav: Path) -> str:
    """Trennen dauert auf einer Mittelklasse-Karte etwa so lang wie das Lied; bei langen Stücken das sagen."""
    import soundfile

    seconds = soundfile.info(str(wav)).duration
    if seconds < 90:
        return text
    return f"{text} – bei {seconds / 60:.0f} min Ton dauert das einige Minuten"


def _peak_db(path: Path) -> float:
    import numpy as np
    import soundfile as sf

    audio, _ = sf.read(str(path), dtype="float32")
    peak = float(np.max(np.abs(audio))) if audio.size else 0.0
    return 20 * np.log10(peak) if peak > 0 else float("-inf")


def _to_wav(source: Path, target: Path) -> None:
    proc.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", str(source), "-ar", "44100", "-ac", "2", str(target)],
        check=True,
    )


def _mix(vocals: Path, instrumental: Path, target: Path) -> None:
    proc.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", str(vocals), "-i", str(instrumental),
         # amix übernähme sonst das Format des RVC-Gesangs (mono, 40 kHz) für die ganze Mischung
         "-filter_complex",
         "[0]aresample=48000,aformat=channel_layouts=stereo[v];[1]aresample=48000,aformat=channel_layouts=stereo[i];"
         "[v][i]amix=inputs=2:duration=longest:normalize=0", str(target)],
        check=True,
    )


def _transpose(source: Path, semitones: int, target: Path) -> Path:
    """Begleitung um Halbtöne verschieben, ohne das Tempo zu ändern (Rubber Band in FFmpeg)."""
    if semitones == 0:
        return source
    proc.run(
        ["ffmpeg", "-y", "-loglevel", "error", "-i", str(source),
         "-af", f"rubberband=pitch={2 ** (semitones / 12):.6f}:pitchq=quality:transients=smooth", str(target)],
        check=True,
    )
    return target


def check_mix_args(args) -> None:
    if args.mix_instrumental and not args.isolate_vocals:
        raise ValueError("--mix-instrumental geht nur zusammen mit --isolate-vocals")
    if not -12 <= args.instrumental_pitch <= 12:
        raise ValueError("--instrumental-pitch muss zwischen -12 und 12 liegen")


def mix_instrumental(sound: Path, instrumental: Path, semitones: int, work: Path) -> Path:
    """Begleitung (falls gewünscht transponiert) unter die neue Stimme bzw. das Instrument mischen."""
    if semitones:
        protocol.progress("transpose", 85, "Begleitung transponieren")
    instrumental = _transpose(instrumental, semitones, work / "instrumental_transposed.wav")
    protocol.progress("mix", 90, "Begleitung dazumischen")
    mixed = work / "mixed.wav"
    _mix(sound, instrumental, mixed)
    return mixed


def _save(source: Path, out: Path) -> None:
    """WAV aus der Arbeit als Ausgabe speichern; andere Formate über FFmpeg."""
    if out.suffix.lower() == ".wav":
        shutil.copyfile(source, out)
    else:
        proc.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(source), str(out)], check=True)


DEMO = Path(__file__).resolve().parent / "assets" / "demo.flac"
# mittlere Tonhöhe des Beispielsatzes (RMVPE), siehe assets/README.md
DEMO_F0 = 193.8


def preview(args) -> None:
    """Hörprobe einer Stimme: den Beispielsatz mit ihr sprechen lassen, passend zu ihrer Tonlage."""
    import math
    from argparse import Namespace

    pitch = 0
    if args.f0 > 0 and DEMO_F0 > 0:
        pitch = max(-12, min(12, round(12 * math.log2(args.f0 / DEMO_F0))))
    convert(Namespace(
        input=str(DEMO), out=args.out, model=args.model, index=args.index, pitch=pitch, formant=1.0,
        index_rate=0.3, protect=0.33, isolate_vocals=False, dereverb=False, denoise=False,
        mix_instrumental=False, instrumental_pitch=0,
    ))


def separate(args) -> None:
    """Gesang und Begleitung trennen (wie Musicfys „Vocal Remover“). Mehr Spuren macht ein eigenes Programm."""
    source = Path(args.input).resolve()
    vocals_out = Path(args.vocals).resolve()
    instrumental_out = Path(args.instrumental).resolve()
    if not source.is_file():
        raise FileNotFoundError(f"Eingabe fehlt: {source}")
    for out in (vocals_out, instrumental_out):
        out.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="tunify_") as tmp:
        work = Path(tmp)
        current = work / "input.wav"
        protocol.progress("load", 2, "Datei einlesen")
        _to_wav(source, current)
        protocol.progress("isolate", 10, _isolate_message("Gesang und Begleitung trennen", current))
        vocals, instrumental = _separate(VOCALS_MODEL, current, work, keep="Vocals")
        if instrumental is None:
            raise RuntimeError("Die Trennung hat keine Begleitspur geliefert")
        protocol.progress("save", 95, "Speichern")
        _save(vocals, vocals_out)
        _save(instrumental, instrumental_out)

    protocol.progress("done", 100, "Fertig")
    protocol.emit("result", vocals=str(vocals_out), instrumental=str(instrumental_out))


def convert(args) -> None:
    source = Path(args.input).resolve()
    out = Path(args.out).resolve()
    model = Path(args.model).resolve()
    index = str(Path(args.index).resolve()) if args.index else ""
    if not source.exists():
        raise FileNotFoundError(f"Eingabe fehlt: {source}")
    if not model.exists():
        raise FileNotFoundError(f"Modell fehlt: {model}")
    check_mix_args(args)
    if not -12 <= args.pitch <= 12:
        raise ValueError("--pitch muss zwischen -12 und 12 liegen")
    if not 0.1 <= args.formant <= 2.0:
        raise ValueError("--formant muss zwischen 0.1 und 2.0 liegen")
    out.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="tunify_") as tmp:
        work = Path(tmp)
        current = work / "input.wav"
        protocol.progress("load", 2, "Datei einlesen")
        _to_wav(source, current)

        instrumental = None
        if args.isolate_vocals:
            protocol.progress("isolate", 10, _isolate_message("Instrumental entfernen", current))
            current, instrumental = _separate(VOCALS_MODEL, current, work, keep="Vocals")
        if args.dereverb:
            protocol.progress("dereverb", 35, "Hall/Echo entfernen")
            current, _ = _separate(DEREVERB_MODEL, current, work, keep="No Reverb")
        if args.denoise:
            protocol.progress("denoise", 50, "Rauschen reduzieren")
            current, _ = _separate(DENOISE_MODEL, current, work, keep="No Noise")

        # RVC stürzt auf der GPU ab, wenn die Spur still ist (z. B. kein Gesang gefunden).
        if _peak_db(current) < SILENCE_DB:
            raise ValueError(
                "Keine Stimme gefunden: Die Spur ist nach dem Vorverarbeiten still. "
                "Enthält die Datei Gesang? Sonst „Instrumental entfernen“ ausschalten."
            )

        protocol.progress("convert", 60, "Stimme umwandeln (RVC)")
        converted = work / "converted.wav"
        core = _applio()
        message, _ = core.run_infer_script(
            pitch=args.pitch,
            index_rate=args.index_rate if index else 0.0,
            volume_envelope=1.0,
            protect=args.protect,
            f0_method="rmvpe",
            input_path=str(current),
            output_path=str(converted),
            pth_path=str(model),
            index_path=index,
            split_audio=False,
            f0_autotune=False,
            f0_autotune_strength=1.0,
            proposed_pitch=False,
            proposed_pitch_threshold=155.0,
            export_format="WAV",
            embedder_model="contentvec",
            formant_shifting=args.formant != 1.0,
            formant_qfrency=args.formant,
            formant_timbre=args.formant,
        )
        if not converted.exists():
            raise RuntimeError(f"RVC hat keine Datei erzeugt: {message}")

        if args.mix_instrumental and instrumental is not None:
            converted = mix_instrumental(converted, instrumental, args.instrumental_pitch, work)

        protocol.progress("save", 97, "Speichern")
        _save(converted, out)

    protocol.progress("done", 100, "Fertig")
    protocol.emit("result", output=str(out))
