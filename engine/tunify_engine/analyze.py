"""Aufnahmen fürs Training bewerten (wie die Qualitätsanzeige bei Musicfys „My Voices“).

Je Datei: Länge, Lautstärke, Übersteuerung, Grundrauschen und Anteil der Pausen → „gut“, „brauchbar“ oder „schlecht“
mit Hinweisen. Nur Messung, nichts wird verändert.
"""

import tempfile
from pathlib import Path

from . import proc, protocol

RATE = 16000
FRAME = 0.05                  # Sekunden je Messfenster
SILENT_DB = -45.0             # leiser gilt als Pause
CLIP_LEVEL = 0.999


def _db(x: float) -> float:
    import math

    return 20 * math.log10(x) if x > 0 else -120.0


def measure(path: Path) -> dict:
    """Messwerte einer Audiodatei."""
    import numpy as np

    with tempfile.TemporaryDirectory(prefix="tunify_analyze_") as tmp:
        raw = Path(tmp) / "a.f32"
        # mono, 16 kHz, rohe Gleitkommazahlen: schnell und ohne Formatfragen
        proc.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(path), "-ac", "1", "-ar", str(RATE),
                  "-f", "f32le", str(raw)], check=True)
        audio = np.fromfile(raw, dtype=np.float32)
    seconds = len(audio) / RATE
    if len(audio) == 0:
        return {"seconds": 0.0, "peak_db": -120.0, "level_db": -120.0, "clipped": 0.0, "noise_db": -120.0,
                "pauses": 1.0}
    size = int(FRAME * RATE)
    frames = audio[: len(audio) // size * size].reshape(-1, size) if len(audio) >= size else audio[None, :]
    frame_db = np.array([_db(float(np.sqrt(np.mean(f ** 2)))) for f in frames])
    voiced = frame_db[frame_db > SILENT_DB]
    return {
        "seconds": round(seconds, 1),
        "peak_db": round(_db(float(np.max(np.abs(audio)))), 1),
        # Lautstärke nur über die Stellen mit Klang, damit Pausen sie nicht herunterziehen
        "level_db": round(float(np.median(voiced)) if voiced.size else -120.0, 1),
        "clipped": round(float(np.mean(np.abs(audio) >= CLIP_LEVEL)) * 100, 3),
        # Grundrauschen: die leisesten 10 % der Fenster
        "noise_db": round(float(np.percentile(frame_db, 10)), 1),
        "pauses": round(float(np.mean(frame_db <= SILENT_DB)), 2),
    }


def rate(m: dict) -> tuple[str, list[str]]:
    """Bewertung und Hinweise aus den Messwerten."""
    problems, hints = 0, []
    if m["seconds"] < 5:
        problems += 2
        hints.append("sehr kurz")
    if m["level_db"] < -35:
        problems += 2
        hints.append("sehr leise – näher ans Mikrofon")
    elif m["level_db"] < -28:
        problems += 1
        hints.append("eher leise")
    if m["clipped"] > 0.5:
        problems += 2
        hints.append("stark übersteuert – leiser aufnehmen")
    elif m["clipped"] > 0.05:
        problems += 1
        hints.append("stellenweise übersteuert")
    # Rauschen nur bewerten, wenn es Pausen gibt, in denen man es hört
    if m["noise_db"] > -40 and m["pauses"] < 0.05:
        hints.append("kaum Pausen – Begleitmusik? Dann „Begleitmusik entfernen“ einschalten")
        problems += 1
    elif m["noise_db"] > -50:
        problems += 1
        hints.append("Rauschen hörbar – „Rauschen reduzieren“ hilft")
    if m["pauses"] > 0.6:
        problems += 1
        hints.append("viele Pausen")
    grade = "gut" if problems == 0 else "brauchbar" if problems <= 1 else "schlecht"
    return grade, hints


def analyze(args) -> None:
    files = []
    for n, name in enumerate(args.inputs, 1):
        path = Path(name).resolve()
        protocol.progress("analyze", round(100 * (n - 1) / len(args.inputs)), f"Aufnahme {n} prüfen")
        try:
            m = measure(path)
            grade, hints = rate(m)
            files.append({"path": str(path), "grade": grade, "hints": hints, **m})
        except Exception as e:  # noqa: BLE001 – eine kaputte Datei soll die anderen nicht verhindern
            files.append({"path": str(path), "grade": "schlecht", "hints": [f"nicht lesbar: {e}"], "seconds": 0.0})
    protocol.emit("result", files=files, seconds=round(sum(f["seconds"] for f in files), 1))
