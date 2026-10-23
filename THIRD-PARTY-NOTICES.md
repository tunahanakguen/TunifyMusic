# Fremde Bausteine

Tunify Music (GPL-3.0) verwendet diese freien Programme und Modelle. Sie liegen weder im Repo noch im Installer:
Die Einrichtung beim ersten Start (`engine/setup.ps1`) lädt sie von den Original-Quellen, jeweils auf eine Version
und SHA-256-Prüfsumme festgelegt, in den Datenordner `%LOCALAPPDATA%\de.tunas.tunifymusic`. Im Repo machen das
`tools/setup-*.ps1`. Einige Modelle lädt der Motor beim ersten Gebrauch nach.

| Baustein | Version | Lizenz | Quelle |
|---|---|---|---|
| Applio (RVC) | 3.7.0 | MIT | https://github.com/IAHispano/Applio |
| python-audio-separator | 0.47.1 | MIT | https://github.com/nomadkaraoke/python-audio-separator |
| PyTorch / torchaudio | 2.11.0 (CUDA 12.8) | BSD-3-Clause | https://pytorch.org |
| Tauri | 2 | MIT / Apache-2.0 | https://tauri.app |
| React | 19 | MIT | https://react.dev |
| FFmpeg | 9.0.2 „full build“ von Gyan (mit Rubber Band) | GPL-3.0 | https://ffmpeg.org, Build: https://github.com/GyanD/codexffmpeg |
| uv | 0.13.1 (lädt Python und Pakete) | MIT / Apache-2.0 | https://github.com/astral-sh/uv |
| Python | 3.12 (über uv) | PSF | https://www.python.org |
| FluidSynth | 2.6.1 (in `engine/.tools`) | LGPL-2.1 | https://github.com/FluidSynth/fluidsynth |
| mido | 1.3.3 | MIT | https://github.com/mido/mido |
| yt-dlp | 2026.08.19 (in `engine/.tools`; aktualisiert sich bei Bedarf selbst) | Unlicense | https://github.com/yt-dlp/yt-dlp |
| ACE-Step 1.5 | v0.1.8 (eigene Umgebung in `engine/vendor/ace-step`, PyTorch 2.7.1 cu128) | MIT | https://github.com/ace-step/ACE-Step-1.5 |

Modelle (Download beim Einrichten bzw. beim ersten Gebrauch):

| Modell | Zweck | Herkunft |
|---|---|---|
| RMVPE, FCPE, ContentVec, HiFi-GAN-Pretrained | RVC-Tonhöhe, Merkmale, Training | Applio-Ressourcen auf Hugging Face (IAHispano/Applio) |
| BS-Roformer (ep_317, viperx) | Gesang/Instrumental trennen | UVR-Modelle über audio-separator |
| UVR-DeEcho-DeReverb, UVR-DeNoise (FoxJoy) | Hall/Echo und Rauschen entfernen | UVR-Modelle über audio-separator |
| **Salamander Grand Piano V3** (SF2 von FreePats, Yamaha C5) | Klavier im Noten-Modus | Alexander Holm, **CC BY 3.0**, https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html |
| ACE-Step 1.5: acestep-v15-turbo (DiT), VAE, acestep-5Hz-lm-0.6B | Lied und Instrumental erzeugen | Hugging Face ACE-Step, **MIT** (`tools/setup-acestep.ps1`, ca. 7,4 GB in `models/ace-step`) |
| Qwen3-Embedding-0.6B (Teil des ACE-Step-Pakets) | Text-Encoder für ACE-Step | Qwen, Apache-2.0 |
| **GeneralUser GS v2.0.3** (SF2, 261 Instrumente) | alle weiteren Instrumente im Noten-Modus | S. Christian Collins, eigene freie Lizenz (Nutzung ohne Einschränkung, auch in Software), https://www.schristiancollins.com/generaluser |
| **LibriTTS-R** (dev-clean) | Beispielsatz der Hörproben (`engine/tunify_engine/assets/demo.flac`, Sprecher 1462) und Trainingsdaten der mitgelieferten Stimmen „Klara“ (Sprecherinnen 84, 7850, 6313) und „Jonas“ (Sprecher 8297, 251, 3000), je 5 min pro Person gemischt | Koizumi et al., Google; nach LibriVox (gemeinfrei); **CC BY 4.0**, https://www.openslr.org/141/ |

Bei Applio gelten zusätzlich dessen Nutzungsbedingungen (`engine/vendor/applio/TERMS_OF_USE.md`):
nur Audio verwenden, an dem man die Rechte hat.
