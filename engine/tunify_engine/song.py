"""Lied und Instrumental erzeugen mit ACE-Step 1.5 (wie Musicfys „Create a Song“ / „Text to Instrumental“).

Beschreibung (+ Liedtext) → ACE-Step (eigener Arbeitsprozess, eigene Umgebung) → WAV
      → [in eigener Stimme: Gesang trennen → RVC → wieder mit der Begleitung mischen] → Ausgabe

Der Arbeitsprozess (engine/acestep_worker.py) wird beim ersten Lied gestartet und bleibt mit geladenen
Modellen bestehen, solange der Motor im Dauerbetrieb läuft.
"""

import json
import shutil
import subprocess
import tempfile
from pathlib import Path

from . import proc, protocol
from .convert import ENGINE_DIR, VOCALS_MODEL, _applio, _mix, _separate, _to_wav

ACE_PYTHON = ENGINE_DIR / "vendor" / "ace-step" / ".venv" / "Scripts" / "python.exe"
WORKER = ENGINE_DIR / "acestep_worker.py"
MODELS = ENGINE_DIR.parent / "models" / "ace-step"

_worker: subprocess.Popen | None = None


def available() -> bool:
    return ACE_PYTHON.exists() and (MODELS / "acestep-v15-turbo").exists()


def _events(process: subprocess.Popen):
    for line in process.stdout:
        try:
            yield json.loads(line)
        except ValueError:
            continue
    raise RuntimeError("ACE-Step wurde unerwartet beendet")


def _ensure_worker() -> subprocess.Popen:
    global _worker
    if _worker is not None and _worker.poll() is None:
        return _worker
    if not available():
        raise FileNotFoundError("ACE-Step ist nicht eingerichtet – bitte tools\\setup-acestep.ps1 ausführen")
    protocol.progress("load", 1, "ACE-Step laden (beim ersten Mal 1–2 Minuten)")
    _worker = proc.popen([str(ACE_PYTHON), "-I", str(WORKER)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, encoding="utf-8", bufsize=1)
    for event in _events(_worker):
        if event["event"] == "ready":
            return _worker
        if event["event"] == "error":
            _worker.kill()
            _worker = None
            raise RuntimeError(event["message"])
    raise RuntimeError("ACE-Step startet nicht")


def _generate(job: dict, share: float) -> dict:
    """Auftrag an ACE-Step; Fortschritt wird auf 0…share Prozent des Gesamtbalkens abgebildet."""
    worker = _ensure_worker()
    worker.stdin.write(json.dumps(job) + "\n")
    worker.stdin.flush()
    result, error = None, None
    for event in _events(worker):
        if event["event"] == "progress":
            protocol.progress("song", event["percent"] * share / 100, event["message"])
        elif event["event"] == "result":
            result = event
        elif event["event"] == "error":
            error = event["message"]
        elif event["event"] == "end":
            break
    if error or result is None:
        raise RuntimeError(error or "ACE-Step hat kein Ergebnis gemeldet")
    return result


def _in_voice(song: Path, work: Path, args) -> Path:
    """Gesang des Liedes mit RVC in die gewählte Stimme umwandeln und wieder zur Begleitung mischen."""
    protocol.progress("isolate", 82, "Gesang von der Begleitung trennen")
    vocals, instrumental = _separate(VOCALS_MODEL, song, work, keep="Vocals")
    protocol.progress("voice", 90, "In deiner Stimme singen")
    converted = work / "vocals_voice.wav"
    core = _applio()
    core.run_infer_script(
        pitch=args.pitch, index_rate=0.3 if args.voice_index else 0.0, volume_envelope=1.0, protect=0.33,
        f0_method="rmvpe", input_path=str(vocals), output_path=str(converted), pth_path=args.voice_model,
        index_path=args.voice_index or "", split_audio=False, f0_autotune=False, f0_autotune_strength=1.0,
        proposed_pitch=False, proposed_pitch_threshold=155.0, export_format="WAV", embedder_model="contentvec",
    )
    if not converted.exists():
        raise RuntimeError("Umwandeln in die Stimme ist fehlgeschlagen")
    protocol.progress("mix", 96, "Gesang und Begleitung mischen")
    mixed = work / "mixed.wav"
    _mix(converted, instrumental, mixed)
    return mixed


def song(args) -> None:
    out = Path(args.out).resolve()
    if not args.caption.strip() and not (args.lyrics or "").strip():
        raise ValueError("Bitte eine Beschreibung oder einen Liedtext angeben")
    if not 10 <= args.duration <= 480:
        raise ValueError("--duration muss zwischen 10 und 480 Sekunden liegen")
    if args.voice_model and args.instrumental:
        raise ValueError("Eine eigene Stimme geht nur bei Liedern mit Gesang")
    reference = Path(args.reference).resolve() if args.reference else None
    if reference and not reference.exists():
        raise FileNotFoundError(f"Referenz-Lied fehlt: {reference}")
    out.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="tunify_song_") as tmp:
        work = Path(tmp)
        if reference:
            # ACE-Step liest nicht jedes Format (z. B. M4A); als WAV geht es immer
            protocol.progress("reference", 1, "Referenz-Lied einlesen")
            _to_wav(reference, work / "reference.wav")
            reference = work / "reference.wav"
        result = _generate({
            "caption": args.caption, "lyrics": args.lyrics or "", "instrumental": args.instrumental,
            "duration": args.duration, "bpm": args.bpm, "keyscale": args.key or "",
            "language": args.language or "unknown", "seed": args.seed, "out_dir": str(work),
            "reference_audio": str(reference) if reference else "",
        }, share=80 if args.voice_model else 97)
        current = Path(result["output"])
        if args.voice_model:
            current = _in_voice(current, work, args)

        protocol.progress("save", 98, "Speichern")
        if out.suffix.lower() == ".wav":
            shutil.copyfile(current, out)
        else:
            proc.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(current), str(out)])

    protocol.progress("done", 100, "Fertig")
    protocol.emit("result", output=str(out), bpm=result.get("bpm"), key=result.get("key"),
                  duration=result.get("duration"), seed=result.get("seed"), lyrics=result.get("lyrics"),
                  seconds=result.get("seconds"))
