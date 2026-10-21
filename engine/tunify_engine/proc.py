"""Unterprozesse (FFmpeg, FluidSynth, Training) ohne Konsolenfenster starten.

Die App startet den Motor ohne Konsole; jedes Konsolenprogramm, das er aufruft, würde sonst
kurz ein eigenes schwarzes Fenster öffnen.
"""

import os
import subprocess

NO_WINDOW = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0


def run(command, **kwargs) -> subprocess.CompletedProcess:
    kwargs.setdefault("check", True)
    return subprocess.run(command, creationflags=NO_WINDOW, **kwargs)


def popen(command, **kwargs) -> subprocess.Popen:
    return subprocess.Popen(command, creationflags=NO_WINDOW, **kwargs)
