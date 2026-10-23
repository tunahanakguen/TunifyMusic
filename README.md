<img src="branding/tunify-logo-1024.png" alt="TUNAS-Logo" width="96">

# Tunify Music

Gesang umwandeln, Stimmen trainieren und Musik erzeugen – komplett lokal unter Windows, ohne Konto und ohne Limit.
Vorbild ist Musicfy; Tunify Music nutzt nur freie Modelle und spricht keine fremden Server an (außer zum
einmaligen Laden der Bausteine und für YouTube-Links).

Hintergrund, Bausteine und Fahrplan: [KONZEPT.md](KONZEPT.md) · Fremde Bausteine und Lizenzen:
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)

## Was es kann

| Bereich | Was passiert |
|---|---|
| **Stimme umwandeln** | Datei, Mikrofon-Aufnahme oder YouTube-Link → mit einer anderen Stimme singen (RVC) oder als Instrument nachspielen (Klavier, Geige, Flöte … aus den Noten). Begleitmusik, Hall und Rauschen entfernen, Begleitung transponiert wieder dazumischen (Cover). Ausgabe als WAV, MP3 oder FLAC, Noten auch als MIDI. |
| **Gesang trennen** | Lied in Gesang und Begleitung aufteilen (Karaoke, Remix). Mehr Spuren trennt ein eigenes TUNAS-Programm. |
| **Stimmen** | Bibliothek: RVC-Stimmen importieren (.pth/.index/.zip), mit Bild und Hörprobe. Zwei freie Stimmen sind dabei. |
| **Stimme trainieren** | Eigene Stimme aus Aufnahmen trainieren; jede Aufnahme wird vorher bewertet (Länge, Pegel, Rauschen …). |
| **Lied erzeugen** | Lied mit Text aus einer Beschreibung (ACE-Step 1.5), optional in einer eigenen Stimme und mit Stilvorlage. |
| **Instrumental erzeugen** | Musik ohne Gesang aus einer Beschreibung, optional mit Stilvorlage. |
| **Verlauf** | Alle Ergebnisse mit Suche, Favoriten und Löschen in den Papierkorb. |

Ergebnisse landen in `Musik\Tunify Music`.

## Systemanforderungen

- Windows 10 oder 11, 64 Bit
- **NVIDIA-Grafikkarte** mit mindestens 4 GB Grafikspeicher (für „Lied/Instrumental erzeugen“ etwa 8 GB) und
  aktuellem Treiber (ab 528.33). Ohne NVIDIA-Karte läuft der KI-Motor nur sehr langsam oder gar nicht – die App
  prüft das vor dem Einrichten.
- Festplatte: ca. 12 GB für den Motor, weitere 14 GB für „Musik erzeugen“
- Internet beim ersten Start (ca. 8 GB bzw. 12 GB Download)

## Installieren

1. `Tunify Music_<Version>_x64-setup.exe` ausführen. Installiert wird für alle Benutzer nach
   `C:\Program Files\Tunify Music` (Windows fragt nach Administratorrechten). Der Installer (ca. 150 MB) enthält
   nur App, Motor-Quellcode und die zwei mitgelieferten Stimmen; den Rest lädt die App beim ersten Start.
2. Beim ersten Start „Jetzt einrichten“: Die App lädt Python, PyTorch, Applio, FFmpeg, FluidSynth, yt-dlp,
   Klangsätze und Modelle – jede Datei auf eine Version und SHA-256-Prüfsumme festgelegt. Dauer 15–40 Minuten.
   Abbrechen ist möglich, „Weitermachen“ setzt dort fort.
3. „Musik erzeugen“ wird erst eingerichtet, wenn man „Lied erzeugen“ oder „Instrumental erzeugen“ öffnet.

Wo was liegt:

| Was | Ort |
|---|---|
| Programm | `C:\Program Files\Tunify Music` |
| KI-Motor, Modelle (veränderlich, groß) | `%LOCALAPPDATA%\de.tunas.tunifymusic` |
| Stimmen-Bibliothek, Favoriten | `%APPDATA%\de.tunas.tunifymusic` |
| Ergebnisse, Aufnahmen, YouTube-Downloads | `Musik\Tunify Music` |

Deinstallieren über Windows-Einstellungen → Apps. Motor, Modelle und Stimmen bleiben liegen, außer man kreuzt beim
Deinstallieren „Lösche die Anwendungsdaten“ an. Die Ergebnisse in `Musik\Tunify Music` bleiben immer erhalten.

## Rechte

Nur Aufnahmen, Lieder, YouTube-Videos und Stimmen verwenden, an denen du die Rechte hast – deine eigenen oder die von
Personen, die zugestimmt haben. Die mitgelieferten Stimmen sind aus freien Hörbuch-Aufnahmen (LibriTTS-R,
CC BY 4.0) gemischt und gehören keiner einzelnen Person.

## Entwickeln

```powershell
powershell -ExecutionPolicy Bypass -File tools\setup-engine.ps1      # Motor in engine\ (Python, Applio, Modelle)
powershell -ExecutionPolicy Bypass -File tools\setup-instruments.ps1 # Klangsätze für den Noten-Modus
powershell -ExecutionPolicy Bypass -File tools\setup-acestep.ps1     # optional: Lied/Instrumental erzeugen
cd app; npm install; npm run tauri dev
```

Im Repo benutzt die App den Motor in `engine\`; ist dort keiner eingerichtet, den aus dem Datenordner.
`TUNIFY_ENGINE_DIR` bzw. `TUNIFY_DATA_DIR` legen beides fest (z. B. zum Testen der Einrichtung in einem leeren Ordner).

Installer bauen: `npx tauri build` in `app\` → `app\src-tauri\target\release\bundle\nsis\`. Die mitgelieferten
Stimmen kommen aus `models\bundled-voices\` (nicht im Repo, zu groß; Herkunft in [KONZEPT.md](KONZEPT.md)).

Motor per Kommandozeile (aus `engine\`, Ausgabe als JSON-Zeilen):

```powershell
.venv\Scripts\python -m tunify_engine convert lied.mp3 --model stimme.pth --isolate-vocals --dereverb --mix-instrumental --out cover.mp3
.venv\Scripts\python -m tunify_engine separate lied.mp3 --vocals gesang.wav --instrumental begleitung.wav
.venv\Scripts\python -m tunify_engine notes summen.wav --instrument geige --out geige.wav --midi geige.mid
.venv\Scripts\python -m tunify_engine song --caption "Lofi, Klavier, Regen" --instrumental --duration 30 --out lofi.wav
.venv\Scripts\python -m tunify_engine youtube https://youtu.be/… --out-dir downloads
```

`python -m tunify_engine --help` zeigt alle Befehle (u. a. `train`, `analyze`, `preview`, `info`).

## Lizenz

Code: GPL-3.0 ([LICENSE](LICENSE)). Das TUNAS-Logo ist das Markenzeichen von Tunahan A. und dient nur zur
Kennzeichnung seiner Programme.
