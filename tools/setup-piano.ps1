# Lädt die Bausteine für Trainingsdaten des Klavier-Modells (Phase 2).
#  - Salamander Grand Piano V3 (SF2, Yamaha C5, CC BY 3.0) von FreePats  → models/instruments/salamander
#  - FluidSynth 2.6.1 (LGPL) von GitHub                                  → engine/.tools/fluidsynth
# Aufruf aus dem Repo-Ordner:  powershell -ExecutionPolicy Bypass -File tools\setup-piano.ps1

$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '..' | Resolve-Path

$sfDir = Join-Path $root 'models\instruments\salamander'
$sf = Join-Path $sfDir 'SalamanderGrandPiano-SF2-V3+20200602\SalamanderGrandPiano-V3+20200602.sf2'
if (-not (Test-Path $sf)) {
    New-Item -ItemType Directory -Force $sfDir | Out-Null
    $archive = Join-Path $sfDir 'SalamanderGrandPiano-SF2-V3+20200602.tar.xz'
    curl.exe -sSL -o $archive 'https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/SalamanderGrandPiano-SF2-V3+20200602.tar.xz'
    tar -xJf $archive -C $sfDir
    Remove-Item $archive
}

$fsDir = Join-Path $root 'engine\.tools\fluidsynth'
$fs = Join-Path $fsDir 'fluidsynth-v2.6.1-win10-x64-cpp11\bin\fluidsynth.exe'
if (-not (Test-Path $fs)) {
    New-Item -ItemType Directory -Force $fsDir | Out-Null
    $zip = Join-Path $fsDir 'fluidsynth.zip'
    curl.exe -sSL -o $zip 'https://github.com/FluidSynth/fluidsynth/releases/download/v2.6.1/fluidsynth-v2.6.1-win10-x64-cpp11.zip'
    Expand-Archive $zip -DestinationPath $fsDir
    Remove-Item $zip
}

& $fs --version | Select-Object -First 1
"Klavier: $sf"
