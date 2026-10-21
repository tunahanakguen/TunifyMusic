"""Stimme trainieren: aus eigenen Aufnahmen ein RVC-Modell machen (wie Musicfys „My Voices“).

Aufnahmen → [Instrumental entfernen] → [Hall/Echo entfernen] → [Rauschen reduzieren] → Datensatz
          → Applio: vorbereiten → Merkmale (RMVPE, ContentVec) → Training (HiFi-GAN, vortrainiert) → Index
          → Ergebnis: model .pth + .index im Arbeitsordner logs/<lauf>/

Die App übernimmt das Ergebnis in ihre Stimmen-Bibliothek und löscht danach den Arbeitsordner.
"""

import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

from . import proc, protocol
from .convert import (APPLIO_DIR, DENOISE_MODEL, DEREVERB_MODEL, VOCALS_MODEL, _applio, _separate, _to_wav)

SAMPLE_RATE = 40000          # wie Musicfy; vortrainierte HiFi-GAN-Modelle gibt es für 32k/40k/48k
MIN_SECONDS = 30.0           # darunter lernt RVC praktisch nichts
EPOCH_LINE = re.compile(r"\| epoch=(\d+) \| step=\d+ \| time=")

# Anteile am Fortschrittsbalken
P_DATA, P_PREPROCESS, P_EXTRACT, P_TRAIN_END = 15, 20, 30, 95


def _seconds(wav: Path) -> float:
    import soundfile as sf

    info = sf.info(str(wav))
    return info.frames / info.samplerate


def _dataset(inputs: list[Path], work: Path, args) -> tuple[Path, float]:
    """Alle Aufnahmen vorverarbeiten und als WAV in einen Datensatz-Ordner legen."""
    dataset = work / "dataset"
    dataset.mkdir()
    total = 0.0
    for n, source in enumerate(inputs, 1):
        protocol.progress("data", round(P_DATA * (n - 1) / len(inputs)), f"Aufnahme {n} von {len(inputs)} vorbereiten")
        step = work / f"in{n}"
        step.mkdir()
        current = step / "input.wav"
        _to_wav(source, current)
        if args.isolate_vocals:
            current, _ = _separate(VOCALS_MODEL, current, step, keep="Vocals")
        if args.dereverb:
            current, _ = _separate(DEREVERB_MODEL, current, step, keep="No Reverb")
        if args.denoise:
            current, _ = _separate(DENOISE_MODEL, current, step, keep="No Noise")
        target = dataset / f"aufnahme_{n:03d}.wav"
        shutil.copyfile(current, target)
        total += _seconds(target)
    return dataset, total


def _train(core, run: str, epochs: int, batch: int) -> None:
    """Applio-Training als Unterprozess; jede Epoche wird als Fortschritt gemeldet."""
    pretrained_g, pretrained_d = core.pretrained_selector("HiFi-GAN", SAMPLE_RATE)
    command = [
        core.python, os.path.join(core.current_script_directory, "rvc", "train", "train.py"),
        run, str(epochs), str(epochs), pretrained_g, pretrained_d, "0", str(batch), str(SAMPLE_RATE),
        # save_only_latest, save_every_weights, cache_data_in_gpu, cleanup, vocoder, checkpointing
        "True", "False", "False", "False", "HiFi-GAN", "False",
    ]
    started = time.time()
    first_epoch_done = None
    process = proc.popen(command, cwd=str(APPLIO_DIR), stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                         text=True, encoding="utf-8", errors="replace", bufsize=1)
    tail: list[str] = []
    for line in process.stdout:
        tail = (tail + [line.rstrip()])[-15:]
        match = EPOCH_LINE.search(line)
        if not match:
            continue
        epoch = int(match.group(1))
        now = time.time()
        # Die erste Epoche dauert wegen Anlaufens länger; Restzeit aus den folgenden schätzen
        if epoch == 1:
            first_epoch_done = now
            remaining = None
        else:
            per_epoch = (now - (first_epoch_done or started)) / max(1, epoch - 1)
            remaining = per_epoch * (epochs - epoch)
        percent = P_EXTRACT + round((P_TRAIN_END - P_EXTRACT) * epoch / epochs)
        message = f"Training: Epoche {epoch} von {epochs}"
        if remaining is not None:
            message += f" · noch etwa {max(1, round(remaining / 60))} min"
        protocol.progress("train", percent, message)
    if process.wait() != 0:
        raise RuntimeError("Training abgebrochen: " + " | ".join(t for t in tail if t)[-500:])


def train(args) -> None:
    inputs = [Path(p).resolve() for p in args.inputs]
    missing = [str(p) for p in inputs if not p.exists()]
    if missing:
        raise FileNotFoundError(f"Aufnahme fehlt: {', '.join(missing)}")
    if not 1 <= args.epochs <= 1000:
        raise ValueError("--epochs muss zwischen 1 und 1000 liegen")

    run = f"tunify-train-{int(time.time())}"
    logs = APPLIO_DIR / "logs" / run
    # Früh melden, damit die App den Arbeitsordner auch nach Abbruch oder Fehler aufräumen kann
    protocol.emit("workspace", path=str(logs))

    with tempfile.TemporaryDirectory(prefix="tunify_train_") as tmp:
        dataset, seconds = _dataset(inputs, Path(tmp), args)
        if seconds < MIN_SECONDS:
            raise ValueError(
                f"Zu wenig Gesang: {seconds:.0f} s. Für eine Stimme braucht es mindestens {MIN_SECONDS:.0f} s, "
                "besser 10 Minuten und mehr."
            )
        protocol.progress("preprocess", P_DATA, f"Datensatz vorbereiten ({seconds / 60:.1f} min Gesang)")
        core = _applio()
        cores = os.cpu_count() or 4
        message = core.run_preprocess_script(
            model_name=run, dataset_path=str(dataset), sample_rate=SAMPLE_RATE, cpu_cores=cores,
            cut_preprocess="Automatic", process_effects=False, chunk_len=3.0, overlap_len=0.3,
            normalization_mode="none",
        )
        if "failed" in message.lower():
            raise RuntimeError(message)

    protocol.progress("extract", P_PREPROCESS, "Merkmale berechnen (Tonhöhe, Klangfarbe)")
    message = core.run_extract_script(
        model_name=run, f0_method="rmvpe", cpu_cores=cores, gpu=0, sample_rate=SAMPLE_RATE,
        embedder_model="contentvec", include_mutes=2,
    )
    if "failed" in message.lower():
        raise RuntimeError(message)

    protocol.progress("train", P_EXTRACT, f"Training: Epoche 0 von {args.epochs}")
    _train(core, run, args.epochs, args.batch)

    protocol.progress("index", P_TRAIN_END, "Index bauen")
    core.run_index_script(run, "Auto")

    models = sorted(logs.glob(f"{run}_*e_*s.pth"), key=lambda p: p.stat().st_mtime)
    if not models:
        raise RuntimeError("Training ist durchgelaufen, aber es wurde kein Modell gespeichert")
    indexes = sorted(logs.glob("*.index"), key=lambda p: not p.name.startswith("added_"))
    protocol.progress("done", 100, "Fertig")
    protocol.emit("result", model=str(models[-1]), index=str(indexes[0]) if indexes else None,
                  workspace=str(logs), seconds=round(seconds, 1))
