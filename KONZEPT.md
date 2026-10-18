# TUNAS Voicify – Konzept

Autor: Tunahan A. · Stand: 18.10.2026 · Phase: **Informationen sammeln**

## Ziel

Eine Windows-App, die Gesang oder eine Melodie in den Klang eines Instruments (zuerst Klavier) oder einer
anderen Stimme umwandelt, so wie „Convert Voice“ bei Musicfy, aber **komplett lokal** auf dem eigenen PC,
ohne Tageslimit und ohne Konto.

TUNAS Voicify ist ein **eigenständiges Projekt**, getrennt vom TuNAS Converter und vom TUNAS Cutter.
Lizenz: GPL-3.0.

**Spielregel:** Wir bauen die *Funktion* nach, nicht den Dienst. Von Musicfy werden keine Modelle,
Programmteile, Grafiken oder Texte übernommen, und die Musicfy-Server werden nicht angesprochen.
Es werden nur freie Modelle und Werkzeuge verwendet.

Analyse von Musicfy: [notizen/musicfy-analyse.md](notizen/musicfy-analyse.md)

**Schwester-App:** [TUNAS Stems](../TunasStems) (Stem-Splitter wie Fadr, eigenes Repo). Das Ziel ist ein
**gemeinsamer Python-Motor** für Trennung (audio-separator/Demucs), FFmpeg und Modellverwaltung.
Voicify ergänzt RVC, Stems ergänzt die Analyse (Tempo, Tonart, Akkorde, MIDI).

## Ablauf (wie Musicfy)

```
Datei / Mikrofon / (YouTube-Link)
  → in WAV umwandeln (FFmpeg)
  → [Instrumental entfernen]       Gesang abtrennen
  → [Hall/Echo entfernen]           DeReverb/DeEcho
  → [Rauschen reduzieren]           Denoise, Stärke 0…1
  → RVC-Umwandlung                  Modell (z. B. Klavier), rmvpe, Pitch ±12, Formant 0,1…2
  → optional: mit Instrumental zusammenmischen
  → WAV/MP3 speichern
```

## Bausteine (alle frei)

| Aufgabe | Kandidat | Bemerkung |
|---|---|---|
| Umwandlung | **RVC v2** über [Applio](https://github.com/IAHispano/Applio) oder `rvc-python` | Gleiches Verfahren wie Musicfy (rmvpe, 40/48 kHz). Applio kann auch trainieren. |
| Tonhöhe | **RMVPE** | Standard bei Musicfy und Applio |
| Gesang trennen | **python-audio-separator** (UVR-Modelle: BS-/Mel-Roformer, MDX) | Beste Qualität derzeit Roformer |
| Hall/Echo | UVR-DeEcho-DeReverb oder Mel-Roformer-DeReverb | über audio-separator |
| Rauschen | UVR-Denoise oder RNNoise / DeepFilterNet | |
| Formant | im RVC-Schritt (Applio hat Formant-Shift) | |
| Ein-/Ausgabe | **FFmpeg** | Ist schon installiert (9.0.2) |
| Klavier-Modell | selbst trainieren aus freien Aufnahmen | z. B. MAESTRO-Datensatz (CC BY-NC-SA, nur privat) oder eigene Aufnahmen |
| Alternative ohne KI | Basic Pitch (Audio → MIDI) + Klavier-Soundfont | Echter Klavierklang, aber ohne Gesangsnuancen. Gut als „Noten-Modus“. |

## Rechner

- NVIDIA GeForce RTX 5070 Laptop (Blackwell, 8 GB) → PyTorch mit **CUDA 12.8 oder neuer** nötig
- AMD Ryzen 7 260, 32 GB RAM
- FFmpeg 9.0.2 vorhanden, **Python fehlt noch**

## Offene Entscheidungen

1. **Programmrahmen**
   - *WPF/.NET* wie der Converter, KI-Teil als gebündeltes Python im Hintergrund (Empfehlung:
     passt zum Converter, Python nur als Rechenmotor)
   - *Electron/React* wie der Cutter
2. **Motor**: Applio als fertiges Paket mitliefern oder eine schlanke eigene Python-Pipeline (rvc-python + audio-separator)?
3. **Modelle**: nur selbst trainierte, oder auch freie Community-Modelle (Lizenz jeweils prüfen)?
4. **Training in der App** (eigene Stimme / eigenes Instrument) gleich mit einplanen oder später?
5. ~~**Name**~~: entschieden am 18.10.2026 → **TUNAS Voicify**

## Fahrplan (Entwurf)

| Phase | Inhalt |
|---|---|
| 0 | Informationen sammeln, Entscheidungen treffen (jetzt) |
| 1 | Machbarkeit: Python + PyTorch (CUDA 12.8) + Applio/RVC einrichten, ein Lied per Kommandozeile umwandeln |
| 2 | Klavier-Modell trainieren und Qualität prüfen |
| 3 | App-Oberfläche: Upload/Aufnahme, drei Schalter, Pitch/Formant, Generieren, Verlauf |
| 4 | Weitere Instrumente, Modellverwaltung, Installer |
