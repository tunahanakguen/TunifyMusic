"""Erzeugt Trainingsdaten fürs Klavier-Modell (Phase 2).

Einstimmige Zufallsmelodien (Tonleitern, Sprünge, Haltetöne, wechselnde Lautstärke) als MIDI,
gerendert mit Salamander Grand Piano über FluidSynth als 48-kHz-WAV.
Tonumfang C2–C6, Schwerpunkt im Bereich von Singstimmen (E2–C5).

Aufruf:  engine\\.venv\\Scripts\\python -I tools\\make-piano-dataset.py [Minuten] [Seed]
Ausgabe: models/instruments/datasets/klavier/*.wav
"""
import os
import random
import subprocess
import sys

import mido

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SF2 = os.path.join(ROOT, r"models\instruments\salamander\SalamanderGrandPiano-SF2-V3+20200602\SalamanderGrandPiano-V3+20200602.sf2")
FLUIDSYNTH = os.path.join(ROOT, r"engine\.tools\fluidsynth\fluidsynth-v2.6.1-win10-x64-cpp11\bin\fluidsynth.exe")
OUT = os.path.join(ROOT, r"models\instruments\datasets\klavier")

TPB = 480
CLIP_SECONDS = 60
SCALES = {
    "dur": [0, 2, 4, 5, 7, 9, 11],
    "moll": [0, 2, 3, 5, 7, 8, 10],
    "pentatonik": [0, 2, 4, 7, 9],
    "chromatisch": list(range(12)),
}
# Tonlagen, aus denen eine Phrase ihre Mitte zieht: Singstimmen-Bereich häufiger
CENTERS = [43, 48, 52, 55, 57, 60, 62, 64, 67, 69, 72, 76, 79]
CENTER_WEIGHTS = [3, 5, 6, 6, 6, 6, 5, 5, 4, 3, 3, 2, 1]
LOW, HIGH = 36, 84  # C2–C6


def scale_notes(rng):
    root = rng.randrange(12)
    steps = SCALES[rng.choices(list(SCALES), weights=[5, 4, 3, 1])[0]]
    return [n for n in range(LOW, HIGH + 1) if (n - root) % 12 in steps]


def phrase(rng, notes):
    """Eine Phrase: Liste von (Note, Dauer in Ticks, Lautstärke, Pause danach)."""
    center = rng.choices(CENTERS, weights=CENTER_WEIGHTS)[0]
    pool = [n for n in notes if abs(n - center) <= 9]
    kind = rng.choices(["melodie", "tonleiter", "arpeggio", "halten"], weights=[6, 2, 2, 2])[0]
    length = rng.randint(4, 14)
    loud = rng.randint(45, 115)
    shape = rng.choice([-1, 0, 1])  # leiser, gleich, lauter werdend
    i = rng.randrange(len(pool))
    out = []
    for k in range(length):
        if kind == "tonleiter":
            i += rng.choice([1, 1, 1, -1]) if k else 0
        elif kind == "arpeggio":
            i += rng.choice([2, 2, -2, 3])
        elif kind == "halten":
            i += rng.choice([-1, 0, 1, 2, -2])
        else:
            i += rng.choices([-4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 7], weights=[1, 2, 5, 9, 2, 9, 5, 2, 1, 1, 1])[0]
        i = max(0, min(len(pool) - 1, i))
        if kind == "halten":
            dur = rng.choice([TPB * 2, TPB * 3, TPB * 4, TPB * 6])
        elif kind in ("tonleiter", "arpeggio"):
            dur = rng.choice([TPB // 4, TPB // 2, TPB // 2, TPB])
        else:
            dur = rng.choices([TPB // 4, TPB // 2, TPB, TPB * 3 // 2, TPB * 2, TPB * 4], weights=[2, 6, 6, 3, 3, 2])[0]
        vel = loud + shape * (k * 40 // length) + rng.randint(-8, 8)
        # meist legato (keine Lücke), manchmal kurz abgesetzt
        gap = 0 if rng.random() < 0.7 else rng.choice([TPB // 8, TPB // 4])
        out.append((pool[i], dur, max(25, min(127, vel)), gap))
    return out


def make_midi(rng, path):
    bpm = rng.randint(60, 140)
    mid = mido.MidiFile(ticks_per_beat=TPB)
    track = mido.MidiTrack()
    mid.tracks.append(track)
    track.append(mido.MetaMessage("set_tempo", tempo=mido.bpm2tempo(bpm)))
    ticks_total = CLIP_SECONDS * bpm * TPB // 60
    ticks, wait = 0, 0
    while ticks < ticks_total:
        notes = scale_notes(rng)
        for note, dur, vel, gap in phrase(rng, notes):
            track.append(mido.Message("note_on", note=note, velocity=vel, time=wait))
            track.append(mido.Message("note_off", note=note, velocity=0, time=dur))
            ticks += wait + dur
            wait = gap
        # Atempause zwischen Phrasen, damit der Ton ausklingen kann
        wait = rng.choice([TPB // 2, TPB, TPB * 2])
    track.append(mido.Message("note_off", note=0, velocity=0, time=TPB * 4))  # Ausklang
    mid.save(path)


def main():
    minutes = float(sys.argv[1]) if len(sys.argv) > 1 else 30
    rng = random.Random(int(sys.argv[2]) if len(sys.argv) > 2 else 2026)
    os.makedirs(OUT, exist_ok=True)
    count = max(1, round(minutes * 60 / CLIP_SECONDS))
    for n in range(count):
        mid = os.path.join(OUT, f"klavier_{n:03d}.mid")
        wav = os.path.join(OUT, f"klavier_{n:03d}.wav")
        make_midi(rng, mid)
        subprocess.run(
            [FLUIDSYNTH, "-ni", "-q", "-g", "1.0", "-r", "48000",
             "-o", "synth.reverb.active=0", "-o", "synth.chorus.active=0",
             "-F", wav, SF2, mid],
            check=True, stdout=subprocess.DEVNULL,
        )
        os.remove(mid)
        print(f"{n + 1}/{count} {os.path.basename(wav)}", flush=True)


if __name__ == "__main__":
    main()
