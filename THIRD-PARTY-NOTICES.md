# Fremde Bausteine

Tunify Music (GPL-3.0) verwendet diese freien Programme und Modelle. Sie liegen nicht im Repo, sondern werden
von `tools/setup-engine.ps1` bzw. beim ersten Gebrauch geladen.

| Baustein | Version | Lizenz | Quelle |
|---|---|---|---|
| Applio (RVC) | 3.7.0 | MIT | https://github.com/IAHispano/Applio |
| python-audio-separator | 0.47.1 | MIT | https://github.com/nomadkaraoke/python-audio-separator |
| PyTorch / torchaudio | 2.11.0 (CUDA 12.8) | BSD-3-Clause | https://pytorch.org |
| Tauri | 2 | MIT / Apache-2.0 | https://tauri.app |
| React | 19 | MIT | https://react.dev |
| FFmpeg | 9.0.2 (systemweit installiert) | LGPL/GPL | https://ffmpeg.org |
| FluidSynth | 2.6.1 (in `engine/.tools`) | LGPL-2.1 | https://github.com/FluidSynth/fluidsynth |
| mido | 1.3.3 | MIT | https://github.com/mido/mido |
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

Bei Applio gelten zusätzlich dessen Nutzungsbedingungen (`engine/vendor/applio/TERMS_OF_USE.md`):
nur Audio verwenden, an dem man die Rechte hat.
