# Tunify Music – Konzept

Autor: Tunahan A. · Stand: 19.10.2026 · Phase: **Informationen sammeln**

## Ziel

Eine Windows-App, die das kann, was Musicfy online anbietet, aber **komplett lokal** auf dem eigenen PC,
ohne Tageslimit, ohne Konto und ohne Kosten pro Minute. Im Mittelpunkt steht „Convert Voice“: Gesang oder eine
Melodie in den Klang eines Instruments (zuerst Klavier) oder einer anderen Stimme umwandeln.

Tunify Music ist ein **eigenständiges Projekt** und teilt nichts mit anderen Apps.
Lizenz: GPL-3.0.

**Spielregel:** Wir bauen die *Funktion* nach, nicht den Dienst. Von Musicfy werden keine Modelle,
Programmteile, Grafiken oder Texte übernommen, und die Musicfy-Server werden nicht angesprochen.
Es werden nur freie Modelle und Werkzeuge verwendet.

Analyse von Musicfy (alle Bereiche): [notizen/musicfy-analyse.md](notizen/musicfy-analyse.md)

## Bereiche der App

| Bereich | Vorbild bei Musicfy | Verfahren | Priorität |
|---|---|---|---|
| **Stimme umwandeln** | Convert Voice | RVC v2 | Kern, zuerst |
| **Stimmen verwalten** | Stimmenliste, Copyright-Free, Instrumentals | Modellordner mit Vorschaubild und Hörprobe | mit dem Kern |
| **Stimme trainieren** | My Voices | RVC-v2-Training | danach |
| **Lied erzeugen** | Create a Song | ACE-Step 1.5, optional danach RVC („in meiner Stimme“) | später |
| **Instrumental erzeugen** | Text to Instrumental | ACE-Step 1.5 (instrumental) | später |
| **Verlauf und Bibliothek** | History, Library | lokale Liste aller Ergebnisse, Favoriten | mit dem Kern |
| Spuren trennen | Stem Splitter | Roformer/Demucs | offen, siehe Entscheidungen |
| API | api.musicfy.lol | – | nicht nötig (alles lokal) |

## Stimme umwandeln (Ablauf wie Musicfy)

```
Datei / Mikrofon / (YouTube-Link)
  → in WAV umwandeln (FFmpeg)
  → [Instrumental entfernen]       Gesang abtrennen
  → [Hall/Echo entfernen]           DeReverb/DeEcho
  → [Rauschen reduzieren]           Denoise, Stärke 0…1
  → RVC-Umwandlung                  Modell (z. B. Klavier), rmvpe, Pitch ±12, Formant 0,1…2
  → optional: Begleitung transponieren (Pitch ±12, Formant) und wieder zusammenmischen („Cover“)
  → WAV/MP3 speichern
```

## Stimme trainieren (wie My Voices)

```
Upload: mehrere Aufnahmen, jede bekommt eine Qualitätsbewertung
  → optional säubern (Gesang abtrennen, Hall/Rauschen entfernen)
Info: Name, Typ (Stimme / Instrument), Abtastrate 40 oder 48 kHz
  → Training: RVC v2, rmvpe, Standard 200 Epochen, Batch an 8 GB VRAM angepasst
  → Vorschau: Hörprobe erzeugen, danach in der Stimmenliste verfügbar
```

## Lied und Instrumental erzeugen (wie Create a Song / Text to Instrumental)

- **Einfach:** Beschreibung, Instrumental ja/nein, Länge 45–240 s.
- **Profi:** eigener Songtext (oder automatisch), Stil, Tempo, Taktart, Tonart, Schreibstil kreativ/wörtlich,
  Referenz-Audio mit Stärke.
- **In meiner Stimme:** Nach ACE-Step wird der Gesang per RVC in eine eigene Stimme umgewandelt, also derselbe Motor
  wie bei „Stimme umwandeln“.
- MusicGen („Classic“ bei Musicfy) kommt **nicht** in Frage: Die Gewichte stehen unter CC BY-NC 4.0.

## Bausteine (alle frei)

| Aufgabe | Kandidat | Bemerkung |
|---|---|---|
| Umwandlung und Training | **RVC v2** über [Applio](https://github.com/IAHispano/Applio) oder `rvc-python` | Gleiches Verfahren wie Musicfy (rmvpe, 40/48 kHz) |
| Tonhöhe | **RMVPE** | Standard bei Musicfy und Applio |
| Gesang trennen | **python-audio-separator** (UVR-Modelle: BS-/Mel-Roformer, MDX) | Beste Qualität derzeit Roformer |
| Hall/Echo | UVR-DeEcho-DeReverb oder Mel-Roformer-DeReverb | über audio-separator |
| Rauschen | UVR-Denoise oder RNNoise / DeepFilterNet | |
| Formant | im RVC-Schritt (Applio hat Formant-Shift) | |
| Lied/Instrumental | **ACE-Step 1.5** (MIT-Lizenz) | 8 GB VRAM: kleines Turbo-Modell bzw. Sparmodus |
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
4. **Umfang der ersten Version**: nur „Stimme umwandeln“ (Empfehlung) oder gleich mit Training bzw. Lied erzeugen?
5. **Spuren trennen**: Die Trennung von Gesang und Begleitung braucht Tunify Music ohnehin. Soll es zusätzlich einen
   eigenen Bereich für vier Spuren geben (Gesang, Schlagzeug, Bass, Rest) wie Musicfys Stem Splitter, oder nicht?
6. ~~**Name**~~: entschieden am 19.10.2026 → **Tunify Music** (vorher TUNAS Musicfyum, TUNAS Voicify)

## Fahrplan (Entwurf)

| Phase | Inhalt |
|---|---|
| 0 | Informationen sammeln, Entscheidungen treffen (jetzt) |
| 1 | Machbarkeit: Python + PyTorch (CUDA 12.8) + Applio/RVC einrichten, ein Lied per Kommandozeile umwandeln |
| 2 | Klavier-Modell trainieren und Qualität prüfen |
| 3 | App-Oberfläche „Stimme umwandeln“: Upload/Aufnahme, drei Schalter, Pitch/Formant, Cover-Mix, Verlauf, Bibliothek |
| 4 | „Stimme trainieren“ in der App |
| 5 | „Lied erzeugen“ und „Instrumental erzeugen“ mit ACE-Step, optional in eigener Stimme |
| 6 | Weitere Instrumente, Installer |
