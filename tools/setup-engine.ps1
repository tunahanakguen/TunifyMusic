# Richtet den Python-Motor von Tunify Music ein (engine/).
# Voraussetzung: uv (winget install astral-sh.uv), Git, FFmpeg im PATH.
# Aufruf aus dem Repo-Ordner:  powershell -ExecutionPolicy Bypass -File tools\setup-engine.ps1

$ErrorActionPreference = 'Stop'
$engine = Join-Path $PSScriptRoot '..\engine' | Resolve-Path
Set-Location $engine

# Applio 3.7.0 (MIT) als RVC-Motor, fest auf diese Version
if (-not (Test-Path 'vendor\applio\core.py')) {
    git clone --depth 1 --branch 3.7.0 https://github.com/IAHispano/Applio.git vendor\applio
}

# Python 3.12 und alle Pakete im Projektordner, nichts systemweit
$env:UV_PYTHON_INSTALL_DIR = Join-Path $engine '.python'
uv python install 3.12
if (-not (Test-Path '.venv\Scripts\python.exe')) { uv venv --python 3.12 .venv }
uv pip install --python .venv -r requirements.txt `
    --extra-index-url https://download.pytorch.org/whl/cu128 --index-strategy unsafe-best-match

# Grundmodelle (RMVPE, ContentVec) und vortrainierte HiFi-GAN-Modelle fürs Training
.\.venv\Scripts\python.exe -m tunify_engine prepare
Push-Location vendor\applio
..\..\.venv\Scripts\python.exe core.py prerequisites --pretraineds-hifigan --no-models --no-exe
Pop-Location

.\.venv\Scripts\python.exe -m tunify_engine info
