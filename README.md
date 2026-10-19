<img src="branding/tunas-logo-1024.png" alt="TUNAS-Logo" width="96">

# Tunify Music

Musicfy lokal nachgebaut: Gesang oder Melodie in den Klang eines Instruments (zuerst Klavier) umwandeln,
eigene Stimmen trainieren und später ganze Lieder erzeugen, komplett lokal unter Windows.
Eigenständiges Projekt, teilt nichts mit anderen Apps.

Hintergrund, Bausteine und Fahrplan: [KONZEPT.md](KONZEPT.md) · Analyse des Vorbilds: [notizen/musicfy-analyse.md](notizen/musicfy-analyse.md)

## Stand

**Phase 1 (Machbarkeit) erledigt am 20.10.2026:**

- Python-Motor (`engine/`): Applio 3.7.0 (RVC) und python-audio-separator auf PyTorch 2.11 mit CUDA 12.8,
  läuft auf der RTX 5070. Kommandozeile mit JSON-Zeilen als Ausgabe: `info`, `prepare`, `convert`
  (Instrumental entfernen, Hall/Echo, Rauschen, Pitch, Formant, Begleitung wieder dazumischen).
- RVC-Training auf der Grafikkarte geprüft (kleines Testmodell).
- App-Gerüst (`app/`): Tauri 2 + React mit TUNAS-Logo, zeigt den Status des KI-Motors.

Technik: Tauri 2 (Rust) + React + Python-Motor, Details in [KONZEPT.md](KONZEPT.md).

## Einrichten

```powershell
powershell -ExecutionPolicy Bypass -File tools\setup-engine.ps1
cd app; npm install; npm run tauri dev
```

Lied per Kommandozeile umwandeln (aus `engine/`):

```powershell
.venv\Scripts\python -m tunify_engine convert lied.mp3 --model stimme.pth --isolate-vocals --dereverb --pitch 0 --mix-instrumental --out cover.mp3
```

## Lizenz

Code: GPL-3.0 ([LICENSE](LICENSE)). Das TUNAS-Logo ist das Markenzeichen von Tunahan A. und dient nur zur
Kennzeichnung seiner Programme.
