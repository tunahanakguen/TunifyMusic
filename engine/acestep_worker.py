"""Arbeitsprozess für ACE-Step 1.5 (Lied und Instrumental erzeugen) – läuft in dessen eigener Umgebung.

Start: engine\\vendor\\ace-step\\.venv\\Scripts\\python -I engine\\acestep_worker.py
Protokoll wie beim Motor: Aufträge als JSON-Zeilen auf stdin, Ereignisse als JSON-Zeilen auf stdout
(ready, progress, result, error, end). Die Modelle bleiben zwischen Aufträgen geladen.

Auftrag: {"caption": "...", "lyrics": "...", "instrumental": false, "duration": 90, "bpm": null,
          "keyscale": "", "language": "de", "seed": -1, "out_dir": "..."}
"""

import json
import os
import sys
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ACE_DIR = ROOT / "engine" / "vendor" / "ace-step"
CHECKPOINTS = ROOT / "models" / "ace-step"
DIT_MODEL = "acestep-v15-turbo"
LM_MODEL = "acestep-5Hz-lm-0.6B"

# Fortschrittstexte von ACE-Step → Anzeige in der App
PHASES = [
    ("Phase 1", "Arrangement planen (Tempo, Tonart, Aufbau)"),
    ("Phase 2", "Musik-Bausteine erzeugen"),
    ("Preparing inputs", "Vorbereiten"),
    ("Decoding audio", "Klang berechnen"),
    ("Preparing audio data", "Fertigstellen"),
]

# Eigener Kanal für unsere Ereignisse; Bibliotheksausgaben gehen nach stderr.
# Unter Windows ist eine umgeleitete Ausgabe cp1252: Zeichen wie „♭“ (Tonart B♭) würden abstürzen.
_out = sys.stdout
sys.stderr.reconfigure(encoding="utf-8", errors="replace")
sys.stdout = sys.stderr


def emit(event: str, **data) -> None:
    # Nur ASCII (Umlaute als \u-Escapes): unabhängig von der Codepage der Pipe
    _out.write(json.dumps({"event": event, **data}) + "\n")
    _out.flush()


def progress(percent: float, message: str) -> None:
    emit("progress", step="song", percent=round(percent, 1), message=message)


def load():
    """ACE-Step mit den Voreinstellungen für die erkannte Grafikkarte laden."""
    os.environ.setdefault("ACESTEP_CHECKPOINTS_DIR", str(CHECKPOINTS))
    os.environ.setdefault("ACESTEP_PROJECT_ROOT", str(ACE_DIR))
    sys.path.insert(0, str(ACE_DIR))
    os.chdir(ACE_DIR)

    from acestep import model_downloader
    # Das große Sprachmodell 1.7B gehört zum „Hauptmodell“; fehlt es, lädt ACE-Step alles neu herunter.
    # Für 8 GB nutzen wir 0.6B und lassen 1.7B bewusst weg.
    if "acestep-5Hz-lm-1.7B" in model_downloader.MAIN_MODEL_COMPONENTS:
        model_downloader.MAIN_MODEL_COMPONENTS.remove("acestep-5Hz-lm-1.7B")

    from acestep.gpu_config import get_gpu_config
    from acestep.handler import AceStepHandler
    from acestep.llm_inference import LLMHandler

    gpu = get_gpu_config()
    dit = AceStepHandler()
    message, ok = dit.initialize_service(
        project_root=str(ACE_DIR),
        config_path=DIT_MODEL,
        device="cuda",
        offload_to_cpu=gpu.offload_to_cpu_default,
        offload_dit_to_cpu=getattr(gpu, "offload_dit_to_cpu_default", False),
        quantization="int8_weight_only" if gpu.quantization_default else None,
        compile_model=getattr(gpu, "compile_model_default", False),
    )
    if not ok:
        raise RuntimeError(f"ACE-Step (DiT) lädt nicht: {message}")
    lm = LLMHandler()
    message, ok = lm.initialize(
        checkpoint_dir=str(CHECKPOINTS), lm_model_path=LM_MODEL, backend="pt", device="cuda",
        offload_to_cpu=gpu.offload_to_cpu_default,
    )
    if not ok:
        raise RuntimeError(f"ACE-Step (Sprachmodell) lädt nicht: {message}")
    return dit, lm


def generate(dit, lm, job: dict) -> dict:
    from acestep.inference import GenerationConfig, GenerationParams, generate_music

    instrumental = bool(job.get("instrumental"))
    params = GenerationParams(
        caption=job.get("caption", ""),
        lyrics="[Instrumental]" if instrumental else job.get("lyrics", ""),
        instrumental=instrumental,
        vocal_language=job.get("language") or "unknown",
        bpm=job.get("bpm") or None,
        keyscale=job.get("keyscale") or "",
        duration=float(job.get("duration") or -1),
        seed=int(job.get("seed", -1)),
    )
    config = GenerationConfig(batch_size=1, use_random_seed=int(job.get("seed", -1)) < 0, audio_format="wav")

    started = time.time()

    def on_progress(value, desc=None, **_):
        # ACE-Step meldet 0…1 und eine englische Beschreibung (wie Gradio)
        text = str(desc or "")
        message = next((de for en, de in PHASES if text.startswith(en)), "Lied entsteht")
        try:
            progress(5 + 90 * float(value), message)
        except (TypeError, ValueError):
            pass

    progress(2, "Arrangement planen")
    result = generate_music(dit, lm, params, config, save_dir=job["out_dir"], progress=on_progress)
    if not result.success:
        raise RuntimeError(result.error or result.status_message or "ACE-Step: unbekannter Fehler")
    audio = result.audios[0]
    meta = audio.get("params", {})
    return {
        "output": audio["path"],
        "seconds": round(time.time() - started, 1),
        "seed": meta.get("seed"),
        # „key“ im Ergebnis ist eine UUID; die Tonart steht in den Parametern (vom Sprachmodell: cot_…)
        "bpm": meta.get("bpm") or meta.get("cot_bpm"),
        "key": meta.get("keyscale") or meta.get("cot_keyscale"),
        "duration": meta.get("duration"),
        "lyrics": meta.get("lyrics") if not instrumental else None,
    }


def main() -> None:
    try:
        dit, lm = load()
    except Exception as error:  # noqa: BLE001 – Fehler gehen als JSON an den Motor
        traceback.print_exc()
        emit("error", message=str(error))
        emit("end")
        return
    emit("ready")
    for line in sys.stdin:
        if not line.strip():
            continue
        try:
            emit("result", **generate(dit, lm, json.loads(line)))
        except Exception as error:  # noqa: BLE001
            traceback.print_exc()
            emit("error", message=str(error))
        emit("end")


if __name__ == "__main__":
    main()
