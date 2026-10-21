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
# Unter Windows ist eine umgeleitete Ausgabe cp1252: Zeichen wie „♭“ würden sonst abstürzen
sys.stderr.reconfigure(encoding="utf-8", errors="replace")
sys.stdout = sys.stderr


def emit(event: str, **data) -> None:
    # Nur ASCII (Umlaute als \u-Escapes): unabhängig von der Codepage der Pipe
    _out.write(json.dumps({"event": event, **data}) + "\n")
    _out.flush()


def progress(step: str, percent: float, message: str = "") -> None:
    emit("progress", step=step, percent=round(percent, 1), message=message)
