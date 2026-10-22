"""Lädt die ACE-Step-Modelle für 8-GB-Grafikkarten (ca. 7,4 GB, MIT) von Hugging Face.

Aufruf (mit der ACE-Step-Umgebung): engine\\vendor\\ace-step\\.venv\\Scripts\\python -I engine\\acestep_download.py <Zielordner>
Bewusst ohne das Sprachmodell 1.7B (3,6 GB): für 8 GB ist 0.6B vorgesehen.
"""
import sys

from huggingface_hub import snapshot_download

target = sys.argv[1]
snapshot_download(
    "ACE-Step/Ace-Step1.5",
    local_dir=target,
    allow_patterns=["acestep-v15-turbo/*", "vae/*", "Qwen3-Embedding-0.6B/*", "config.json"],
)
snapshot_download("ACE-Step/acestep-5Hz-lm-0.6B", local_dir=f"{target}/acestep-5Hz-lm-0.6B")
print("Modelle bereit:", target)
