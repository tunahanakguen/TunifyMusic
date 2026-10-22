# Einrichtung von Tunify Music beim ersten Start (die App ruft das auf, mit Fortschrittsbalken).
#
#   setup.ps1 -Root <Datenordner> -Part core    Motor: Python, Pakete, Applio, Modelle, FFmpeg, FluidSynth, Klangsätze
#   setup.ps1 -Root <Datenordner> -Part music   Lied/Instrumental erzeugen: ACE-Step mit eigener Umgebung und Modellen
#
# <Datenordner>\engine enthält schon den Motor-Quellcode (die App kopiert ihn hinein). Alles Weitere landet in
# diesem Ordner, nichts systemweit. Jede Datei ist auf eine Version und eine SHA-256-Prüfsumme festgelegt.
# Abgebrochene Einrichtung: einfach erneut starten, Fertiges wird übersprungen.
# Fortschritt auf stdout als Zeilen „@@ <Prozent> <Text>“.

param(
    [Parameter(Mandatory)][string]$Root,
    [ValidateSet('core', 'music')][string]$Part = 'core'
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

# Python-Pakete haben tief verschachtelte Dateien; ohne „lange Pfade“ in Windows sind 260 Zeichen die Grenze
$longPaths = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -ErrorAction SilentlyContinue).LongPathsEnabled
if ($Root.Length -gt 90 -and $longPaths -ne 1) {
    throw "Der Datenordner hat einen zu langen Pfad ($($Root.Length) Zeichen, höchstens 90): $Root"
}

$engine = Join-Path $Root 'engine'
$models = Join-Path $Root 'models'
$tools = Join-Path $engine '.tools'
$downloads = Join-Path $Root 'downloads'
New-Item -ItemType Directory -Force $tools, $models, $downloads | Out-Null

$env:UV_PYTHON_INSTALL_DIR = Join-Path $engine '.python'
$env:UV_LINK_MODE = 'copy'
$env:PYTHONUTF8 = '1'

$SOURCES = @{
    uv          = @{ url = 'https://github.com/astral-sh/uv/releases/download/0.13.1/uv-x86_64-pc-windows-msvc.zip'
                     sha = '3B2661A463A5BAEA7F5076EA2B25AB71D77F36A6C0D809E5350651792F80A672' }
    ffmpeg      = @{ url = 'https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-full_build.zip'
                     sha = '759D0A9831C436A0EB331AD36F236C06CB04AAA0005DA46571F0E9D3D9206F6B' }
    applio      = @{ url = 'https://github.com/IAHispano/Applio/archive/refs/tags/3.7.0.zip'
                     sha = '856A1F3FC7A402B971BF805674F067EE8E06AC92DE7CEE90F2E1A4B3D4B6D79E' }
    fluidsynth  = @{ url = 'https://github.com/FluidSynth/fluidsynth/releases/download/v2.6.1/fluidsynth-v2.6.1-win10-x64-cpp11.zip'
                     sha = 'FAB7A2E4B85675B66970F97A39BBC239729C5E0F237198B5922A6A73CBC8677C' }
    salamander  = @{ url = 'https://freepats.zenvoid.org/Piano/SalamanderGrandPiano/SalamanderGrandPiano-SF2-V3+20200602.tar.xz'
                     sha = '15EDB061D7BA60D58332F72DBA8F8CE40988048CC703F935E6320F37D650E213' }
    generaluser = @{ url = 'https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/684543d5e5efaef08d02be50dcda8d552478fa60/GeneralUser-GS.sf2'
                     sha = '9575028C7A1F589F5770FCCC8CFF2734566AF40CD26ED836944E9A5152688CFE' }
    gu_license  = @{ url = 'https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/684543d5e5efaef08d02be50dcda8d552478fa60/documentation/LICENSE.txt'
                     sha = '7B32EFEFDF95CE38A043799F0659853DDC00FBAA14D8C50F0ACA16B9B8B405BE' }
    acestep     = @{ url = 'https://github.com/ace-step/ACE-Step-1.5/archive/refs/tags/v0.1.8.zip'
                     sha = '8C39E3E4C7C939ED8D3617E4FBE81585EC869276A228DA5D6080BB2C3E4C3DB0' }
}

function Step([int]$percent, [string]$text) {
    Write-Output "@@ $percent $text"
}

# Lädt eine Quelle (mit Wiederholungen) und prüft die Prüfsumme. Gibt den Dateipfad zurück.
function Fetch([string]$name) {
    $source = $SOURCES[$name]
    $file = Join-Path $downloads $name
    if ((Test-Path $file) -and (Get-FileHash $file -Algorithm SHA256).Hash -eq $source.sha) { return $file }
    & curl.exe -sSfL --retry 5 --retry-delay 3 --retry-all-errors -o $file $source.url
    if ($LASTEXITCODE -ne 0) { throw "Download fehlgeschlagen: $($source.url)" }
    $hash = (Get-FileHash $file -Algorithm SHA256).Hash
    if ($hash -ne $source.sha) {
        Remove-Item $file
        throw "Prüfsumme stimmt nicht: $($source.url)"
    }
    return $file
}

# Entpackt ein Archiv mit genau einem Wurzelordner nach $target (ohne diesen Wurzelordner).
function Unpack([string]$archive, [string]$target, [string]$flags = '-xf') {
    $tmp = "$target.tmp"
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
    New-Item -ItemType Directory -Force $tmp | Out-Null
    & tar $flags $archive -C $tmp
    if ($LASTEXITCODE -ne 0) { throw "Entpacken fehlgeschlagen: $archive" }
    $inner = Get-ChildItem $tmp
    $from = if ($inner.Count -eq 1 -and $inner[0].PSIsContainer) { $inner[0].FullName } else { $tmp }
    if (Test-Path $target) { Remove-Item -Recurse -Force $target }
    Move-Item $from $target
    if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
}

function Run([string]$exe) {
    & $exe @args
    if ($LASTEXITCODE -ne 0) { throw "Fehlgeschlagen ($LASTEXITCODE): $exe $args" }
}

# uv und FFmpeg brauchen beide Teile
$uv = Join-Path $tools 'uv\uv.exe'
if (-not (Test-Path $uv)) {
    Step 1 'Werkzeuge laden (uv)'
    Unpack (Fetch 'uv') (Join-Path $tools 'uv')
}
$ffmpeg = Join-Path $tools 'ffmpeg\bin'
if (-not (Test-Path (Join-Path $ffmpeg 'ffmpeg.exe'))) {
    Step 3 'FFmpeg laden'
    Unpack (Fetch 'ffmpeg') (Join-Path $tools 'ffmpeg')
}
$env:Path = "$ffmpeg;$env:Path"

if ($Part -eq 'core') {
    $applio = Join-Path $engine 'vendor\applio'
    if (-not (Test-Path (Join-Path $applio 'core.py'))) {
        Step 8 'Applio (RVC) laden'
        New-Item -ItemType Directory -Force (Join-Path $engine 'vendor') | Out-Null
        Unpack (Fetch 'applio') $applio
    }

    $fluidsynth = Join-Path $tools 'fluidsynth'
    if (-not (Test-Path (Join-Path $fluidsynth 'fluidsynth-v2.6.1-win10-x64-cpp11\bin\fluidsynth.exe'))) {
        Step 10 'FluidSynth laden'
        # notes.py erwartet den Ordner mit Versionsnamen darunter
        New-Item -ItemType Directory -Force $fluidsynth | Out-Null
        Unpack (Fetch 'fluidsynth') (Join-Path $fluidsynth 'fluidsynth-v2.6.1-win10-x64-cpp11')
    }

    $salamander = Join-Path $models 'instruments\salamander'
    if (-not (Test-Path (Join-Path $salamander 'SalamanderGrandPiano-SF2-V3+20200602\SalamanderGrandPiano-V3+20200602.sf2'))) {
        Step 12 'Klavier-Klangsatz laden (Salamander, 300 MB)'
        New-Item -ItemType Directory -Force $salamander | Out-Null
        Unpack (Fetch 'salamander') (Join-Path $salamander 'SalamanderGrandPiano-SF2-V3+20200602') '-xJf'
    }
    $gu = Join-Path $models 'instruments\generaluser'
    if (-not (Test-Path (Join-Path $gu 'GeneralUser-GS.sf2'))) {
        Step 16 'Instrumente laden (GeneralUser GS)'
        New-Item -ItemType Directory -Force $gu | Out-Null
        Copy-Item (Fetch 'gu_license') (Join-Path $gu 'LICENSE.txt')
        Copy-Item (Fetch 'generaluser') (Join-Path $gu 'GeneralUser-GS.sf2')
    }

    $python = Join-Path $engine '.venv\Scripts\python.exe'
    if (-not (Test-Path (Join-Path $engine '.venv\installed.txt'))) {
        Step 20 'Python einrichten'
        Run $uv python install 3.12
        if (-not (Test-Path $python)) { Run $uv venv --python 3.12 (Join-Path $engine '.venv') }
        Step 25 'KI-Pakete laden (PyTorch mit CUDA, ca. 4 GB – das dauert)'
        Run $uv pip install --python (Join-Path $engine '.venv') -r (Join-Path $engine 'requirements.lock.txt') `
            --extra-index-url https://download.pytorch.org/whl/cu128 --index-strategy unsafe-best-match
        Set-Content (Join-Path $engine '.venv\installed.txt') (Get-Date -Format s)
    }

    Push-Location $engine
    try {
        Step 75 'Grundmodelle laden (RMVPE, ContentVec)'
        Run $python -m tunify_engine prepare | Out-Null
        Step 82 'Trainingsmodelle laden (HiFi-GAN)'
        Push-Location $applio
        try { Run $python core.py prerequisites --pretraineds-hifigan --no-models --no-exe | Out-Null }
        finally { Pop-Location }
        Step 92 'Gesang-Trennung vorbereiten (BS-Roformer)'
        Run $python -c "from tunify_engine.convert import SEPARATOR_MODELS_DIR, VOCALS_MODEL; from audio_separator.separator import Separator; SEPARATOR_MODELS_DIR.mkdir(parents=True, exist_ok=True); Separator(model_file_dir=str(SEPARATOR_MODELS_DIR)).load_model(model_filename=VOCALS_MODEL)" | Out-Null
        Step 98 'Prüfen'
        Run $python -m tunify_engine info | Out-Null
    }
    finally { Pop-Location }
}
else {
    $ace = Join-Path $engine 'vendor\ace-step'
    if (-not (Test-Path (Join-Path $ace 'uv.lock'))) {
        Step 5 'ACE-Step laden'
        New-Item -ItemType Directory -Force (Join-Path $engine 'vendor') | Out-Null
        Unpack (Fetch 'acestep') $ace
    }
    if (-not (Test-Path (Join-Path $ace '.venv\installed.txt'))) {
        Step 10 'Pakete für ACE-Step laden (ca. 5 GB – das dauert)'
        Push-Location $ace
        try { Run $uv sync --frozen --python 3.12 }
        finally { Pop-Location }
        Set-Content (Join-Path $ace '.venv\installed.txt') (Get-Date -Format s)
    }
    Step 45 'Musikmodelle laden (ca. 7,4 GB – das dauert)'
    $target = Join-Path $models 'ace-step'
    New-Item -ItemType Directory -Force $target | Out-Null
    Run (Join-Path $ace '.venv\Scripts\python.exe') -I (Join-Path $engine 'acestep_download.py') $target
}

# Downloads werden nicht mehr gebraucht (bei einem Neustart nach Fehler bleiben sie liegen)
Remove-Item -Recurse -Force $downloads -ErrorAction SilentlyContinue
# Erst dieses Zeichen schaltet die App auf den eingerichteten Motor um
Set-Content (Join-Path $Root "ready-$Part.txt") (Get-Date -Format s)
Step 100 'Fertig'
