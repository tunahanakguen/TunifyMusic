"""Tonspur eines YouTube-Videos laden (wie Musicfys YouTube-Link), mit yt-dlp aus engine/.tools.

Nur einzelne Videos von YouTube, höchstens 20 Minuten. Die Datei landet im Ausgabeordner unter
„Downloads“ und geht dann wie jede andere Audiodatei in Umwandeln oder Trennen.
"""

import re
import subprocess
from pathlib import Path
from urllib.parse import urlparse

from . import protocol

ENGINE_DIR = Path(__file__).resolve().parent.parent
YT_DLP = ENGINE_DIR / ".tools" / "yt-dlp" / "yt-dlp.exe"
HOSTS = {"youtube.com", "www.youtube.com", "m.youtube.com", "music.youtube.com", "youtu.be"}
MAX_SECONDS = 20 * 60
NO_WINDOW = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def check_url(url: str) -> str:
    url = url.strip()
    parsed = urlparse(url)
    if parsed.scheme not in ("http", "https") or parsed.hostname not in HOSTS:
        raise ValueError("Bitte einen Link zu einem YouTube-Video einfügen (youtube.com oder youtu.be)")
    return url


def _download(url: str, out_dir: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(YT_DLP), url, "--no-playlist", "--no-progress", "--newline",
         "-f", "bestaudio/best", "--match-filter", f"duration <= {MAX_SECONDS}",
         "--windows-filenames", "--no-mtime",
         "-o", str(out_dir / "%(title).80s.%(ext)s"),
         "--print", "after_move:filepath"],
        capture_output=True, text=True, encoding="utf-8", errors="replace", creationflags=NO_WINDOW,
    )


def _reason(result: subprocess.CompletedProcess) -> str:
    errors = [line for line in result.stderr.splitlines() if line.startswith("ERROR")]
    return re.sub(r"^ERROR:\s*(\[\w+\]\s*\S+:\s*)?", "", errors[-1]) if errors else result.stderr.strip()[-300:]


def youtube(args) -> None:
    url = check_url(args.url)
    out_dir = Path(args.out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    if not YT_DLP.exists():
        raise FileNotFoundError(f"yt-dlp fehlt: {YT_DLP} – bitte die Einrichtung wiederholen")

    protocol.progress("download", 10, "Tonspur von YouTube laden")
    result = _download(url, out_dir)
    if result.returncode != 0 and "does not pass filter" not in result.stdout + result.stderr:
        # YouTube ändert sich oft; yt-dlp aktualisiert sich selbst (prüft Prüfsumme und Signatur) und versucht es neu
        protocol.progress("update", 40, "yt-dlp aktualisieren und neu versuchen")
        subprocess.run([str(YT_DLP), "-U"], capture_output=True, creationflags=NO_WINDOW)
        result = _download(url, out_dir)

    files = [Path(line.strip()) for line in result.stdout.splitlines() if line.strip()]
    if not files or not files[-1].exists():
        if "does not pass filter" in result.stdout + result.stderr:
            raise ValueError("Das Video ist länger als 20 Minuten")
        raise RuntimeError(f"YouTube-Download fehlgeschlagen: {_reason(result)}")
    path = files[-1]
    protocol.progress("done", 100, "Fertig")
    protocol.emit("result", path=str(path), title=path.stem)
