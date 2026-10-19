"""JSON-Zeilen auf stdout: so spricht der Motor mit der App.

Ereignisse:
  {"event": "progress", "step": "...", "percent": 0-100, "message": "..."}
  {"event": "result", ...}
  {"event": "error", "message": "..."}
"""

import json
import sys

# Bibliotheken schreiben gern selbst auf stdout. Damit die App nur saubere JSON-Zeilen
# bekommt, gehen unsere Ereignisse über einen eigenen Kanal und alles andere nach stderr.
_out = sys.stdout
sys.stdout = sys.stderr


def emit(event: str, **data) -> None:
    _out.write(json.dumps({"event": event, **data}, ensure_ascii=False) + "\n")
    _out.flush()


def progress(step: str, percent: float, message: str = "") -> None:
    emit("progress", step=step, percent=round(percent, 1), message=message)
