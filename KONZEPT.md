# Tunify Music – Konzept

Autor: Tunahan A. · Stand: 20.10.2026 · Phase: **1 erledigt, als Nächstes 2**

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

## Technik (entschieden am 20.10.2026)

**Tauri 2**: Rust-Kern + Web-Oberfläche + Python-Motor im Hintergrund.

| Schicht | Wahl | Aufgabe |
|---|---|---|
| Programmrahmen | **Tauri 2** (Rust) | Fenster, Dateien, Installer; nutzt das in Windows 11 eingebaute WebView2 |
| Oberfläche | **React + TypeScript** (Vite) | Musicfy-artige Bedienung, Wellenformen, Regler; TUNAS-Design mit **Rot (#C8102E) als Erkennungsfarbe**, Design Hell (Standard), Dunkel oder „Wie Windows“ |
| Rust-Kern | Tauri-Befehle | Audio aufnehmen/abspielen (`cpal`, `symphonia`), FFmpeg aufrufen, Python-Motor starten und überwachen, Verlauf und Einstellungen speichern |
| KI-Motor | **Python 3.11 + PyTorch (CUDA 12.8+)** als eigener Prozess | Trennen, Hall/Rauschen, RVC-Umwandlung und -Training, später ACE-Step |
| Verbindung | JSON-Zeilen über stdin/stdout | Aufträge, Fortschritt in Prozent, Ergebnisse, Fehler (wie FFmpeg beim Converter) |
| Python-Umgebung | **uv** | Python und Pakete in einen eigenen Ordner der App, nichts systemweit |
| Später | **ONNX Runtime in Rust** (`ort`, CUDA) | Trennen und RVC-Umwandlung ohne Python, damit der Kern ohne mehrere GB Python läuft; Training und ACE-Step bleiben in Python |

Warum Tauri: Die KI läuft ohnehin auf der Grafikkarte. Tauri hält die App selbst klein (ca. 40–90 MB RAM,
Installer ohne KI ca. 5–15 MB) und lässt mehr Grafikspeicher für die Modelle übrig als Electron.

Geplanter Aufbau des Repos:

```
TunifyMusic/
  app/            Tauri-2-App: src/ (React-Oberfläche), src-tauri/ (Rust-Kern)
  engine/         Python-Motor (Paket tunify_engine, pyproject.toml für uv)
  models/         Modelle, nicht im Repo
  branding/  notizen/  KONZEPT.md  README.md  LICENSE
```

## Rechner und Werkzeuge

- NVIDIA GeForce RTX 5070 Laptop (Blackwell, 8 GB), Treiber 595.97 → PyTorch mit **CUDA 12.8 oder neuer**
- AMD Ryzen 7 260, 32 GB RAM
- Werkzeuge: FFmpeg 9.0.2, Node 24.20, Git 2.55, WebView2 155, Rust 1.99.1, VS Build Tools 2022 (C++), uv 0.13.1
- Python 3.12.15 und alle Pakete liegen im Projekt (`engine/.python`, `engine/.venv`), PyTorch 2.11.0+cu128 erkennt die RTX 5070 (sm_120)

## Offene Entscheidungen

1. ~~**Programmrahmen**~~: entschieden am 20.10.2026 → **Tauri 2 + React + Python-Motor**, später ONNX in Rust
2. ~~**Motor**~~: entschieden am 20.10.2026 → **Applio 3.7.0** (MIT, fest auf diese Version, in `engine/vendor`) für RVC-Umwandlung und -Training + **python-audio-separator 0.47.1** (BS-Roformer, UVR-DeEcho-DeReverb, UVR-DeNoise), angesteuert vom eigenen Paket `tunify_engine`
3. **Modelle**: nur selbst trainierte, oder auch freie Community-Modelle (Lizenz jeweils prüfen)?
   Stand 21.10.2026: Die App bringt keine Stimmen mit; man importiert eigene .pth/.index/.zip (Hinweis auf
   Rechte/Einwilligung im Import-Dialog). Ob Tunify Music selbst Stimmen mitliefert, ist noch offen.
4. **Umfang der ersten Version**: nur „Stimme umwandeln“ (Empfehlung) oder gleich mit Training bzw. Lied erzeugen?
5. **Spuren trennen**: Die Trennung von Gesang und Begleitung braucht Tunify Music ohnehin. Soll es zusätzlich einen
   eigenen Bereich für vier Spuren geben (Gesang, Schlagzeug, Bass, Rest) wie Musicfys Stem Splitter, oder nicht?
6. ~~**Name**~~: entschieden am 19.10.2026 → **Tunify Music** (vorher TUNAS Musicfyum, TUNAS Voicify)

## Fahrplan (Entwurf)

| Phase | Inhalt |
|---|---|
| 0 | Informationen sammeln, Entscheidungen treffen ✓ |
| 1 | Werkzeuge installieren (Rust, Build Tools, uv/Python). Python-Motor: PyTorch (CUDA 12.8) + RVC, ein Lied per Kommandozeile umwandeln. Leeres Tauri-2-Gerüst mit TUNAS-Logo ✓ (20.10.2026) |
| 2 | Klavier-Modell trainieren und Qualität prüfen ✓ (21.10.2026) → Ergebnis: Klavier über den **Noten-Modus**, RVC nur für Stimme → Stimme |
| 3 | App-Oberfläche „Stimme umwandeln“: Upload/Aufnahme, drei Schalter, Pitch/Formant, Cover-Mix, Verlauf, Bibliothek ✓ (21.10.2026): Instrument (Noten-Modus, 18 Instrumente) und Stimme (RVC), Mikrofon-Aufnahme, WAV/MP3/FLAC, MIDI, Abbrechen, Verlauf, Stimmen-Bibliothek mit Import (.pth/.index/.zip) |
| 4 | „Stimme trainieren“ in der App ✓ (21.10.2026): Aufnahmen hinzufügen/aufnehmen, Vorverarbeitung, Schnell/Standard/Gründlich (100/200/400 Epochen, 40 kHz), Fortschritt je Epoche mit Restzeit, Abbrechen, Ergebnis direkt in der Bibliothek; 1,4 min Gesang × 100 Epochen ≈ 11 min |
| 5 | „Lied erzeugen“ und „Instrumental erzeugen“ mit ACE-Step, optional in eigener Stimme — Motor und Seiten fertig (21.10.2026), Test in der App offen. ACE-Step 1.5 v0.1.8 in eigener Umgebung (PyTorch 2.7.1), Turbo-DiT + Sprachmodell 0.6B (8-GB-Stufe, INT8, Auslagern), eigener Arbeitsprozess hält die Modelle. Gemessen: Laden 70–90 s beim ersten Mal, 30 s Musik in ca. 55 s; „in eigener Stimme“: Gesang trennen → RVC → mischen |
| 6 | Weitere Instrumente, Installer |

## Erkenntnisse aus Phase 1 (20.10.2026)

- Ganze Kette per Kommandozeile geprüft: Trennen → Hall/Echo → Rauschen → RVC (Pitch +3, Formant 1,2, Index)
  → Begleitung dazumischen → MP3. 20 s Audio in ca. 55 s inklusive erstem Laden aller Modelle; ohne
  Vorverarbeitung ca. 30 s. Die Modelle beim App-Start vorzuladen spart später den größten Teil.
- RVC-Training läuft auf der RTX 5070: 1 min Audio, 5 Epochen, Batch 4 → ca. 3 min inklusive Index.
- **Stille Spur bringt RVC zum Absturz** (CUDA device-side assert), z. B. wenn „Instrumental entfernen“ keinen
  Gesang findet. Der Motor prüft das jetzt vorher und meldet „Keine Stimme gefunden“.
- Die Trennmodelle benennen ihre Spuren `(Vocals)/(Instrumental)`, `(No Reverb)/(Reverb)`, `(No Noise)/(Noise)`.
- Grundmodelle: RMVPE, FCPE, ContentVec (ca. 0,6 GB) und vortrainierte HiFi-GAN-Modelle fürs Training (ca. 1,3 GB).

## Erkenntnisse aus Phase 2 (21.10.2026)

**Entscheidung: Instrumente laufen über den Noten-Modus, RVC bleibt für Stimme → Stimme.**

- RVC-Klaviermodell trainiert: 36,6 min Salamander-Melodien (C2–C6), 48 kHz, RMVPE, 300 Epochen, Batch 8 →
  knapp 4 h auf der RTX 5070 (erste Epoche 3:18 min, danach 35–55 s). Ergebnis bei allen Zwischenständen:
  „Gesang mit Klavierfarbe“, kein Klavier. RVC behält Lautstärkeverlauf, Gleittöne, Vibrato und Silben der
  Stimme bei; ein Klavier hat Hammeranschlag, Ausklingen und feste Tonhöhen. Das ist eine Grenze des
  Verfahrens, nicht der Trainingsdauer.
- **Noten-Modus** (`tunify_engine notes`): RMVPE-Tonhöhe → Noten (kurze Übergänge zusammenfassen, Stimmung
  ausgleichen, auf erkannte Tonart einrasten, Anschlag aus der Lautstärke) → MIDI → Salamander Grand Piano über
  FluidSynth. Klingt nach echtem Klavier, braucht kein Training, dauert Sekunden. Gleittöne und Vibrato gehen
  verloren. Weitere Instrumente brauchen nur einen weiteren Klangsatz (SF2).
- Eigene Testaufnahmen (Männerstimme, ca. F#2–H3): in Originaltonlage klingt es besser als eine Oktave höher.
