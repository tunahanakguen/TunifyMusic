# Richtet ACE-Step 1.5 ein (Lied und Instrumental erzeugen, Phase 5).
#  - Code:    ACE-Step-1.5 v0.1.8 (MIT)               → engine/vendor/ace-step   (eigene Umgebung .venv, PyTorch 2.7.1 cu128)
#  - Modelle: Hugging Face ACE-Step (MIT), ca. 7,4 GB → models/ace-step
#      acestep-v15-turbo (DiT), vae, Qwen3-Embedding-0.6B (Text-Encoder), acestep-5Hz-lm-0.6B (Sprachmodell)
#    Das große Sprachmodell 1.7B (3,6 GB) wird bewusst nicht geladen: für 8-GB-Grafikkarten ist 0.6B vorgesehen.
# Aufruf aus dem Repo-Ordner:  powershell -ExecutionPolicy Bypass -File tools\setup-acestep.ps1

$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '..' | Resolve-Path
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
$env:UV_PYTHON_INSTALL_DIR = Join-Path $root 'engine\.python'

$ace = Join-Path $root 'engine\vendor\ace-step'
if (-not (Test-Path $ace)) {
    git clone -q --depth 1 --branch v0.1.8 https://github.com/ace-step/ACE-Step-1.5.git $ace
}
Push-Location $ace
uv sync --frozen --python 3.12
Pop-Location

$models = Join-Path $root 'models\ace-step'
New-Item -ItemType Directory -Force $models | Out-Null
$python = Join-Path $ace '.venv\Scripts\python.exe'
& $python -I (Join-Path $root 'engine\acestep_download.py') $models
