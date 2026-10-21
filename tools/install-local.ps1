# Installiert die gebaute App auf diesem PC (ohne Installer) und legt Verknüpfungen an.
#  - Programm:   %LOCALAPPDATA%\Programs\Tunify Music\Tunify Music.exe
#  - Startmenü:  Tunify Music
#  - Desktop:    Tunify Music
# Der Python-Motor bleibt im Repo (engine/); sein Pfad steckt beim Bauen in der App.
# Vorher bauen:  cd app; npx tauri build --no-bundle
# Aufruf:        powershell -ExecutionPolicy Bypass -File tools\install-local.ps1

$ErrorActionPreference = 'Stop'
$root = Join-Path $PSScriptRoot '..' | Resolve-Path
$built = Join-Path $root 'app\src-tauri\target\release\tunify-music.exe'
if (-not (Test-Path $built)) { throw "Erst bauen: cd app; npx tauri build --no-bundle" }

$target = Join-Path $env:LOCALAPPDATA 'Programs\Tunify Music'
$exe = Join-Path $target 'Tunify Music.exe'
if (Get-Process -Name 'Tunify Music' -ErrorAction SilentlyContinue) { throw "Tunify Music läuft noch – bitte erst schließen." }
New-Item -ItemType Directory -Force $target | Out-Null
Copy-Item $built $exe -Force

$shell = New-Object -ComObject WScript.Shell
$links = @(
    (Join-Path ([Environment]::GetFolderPath('Programs')) 'Tunify Music.lnk'),
    (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Tunify Music.lnk')
)
foreach ($path in $links) {
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = $exe
    $link.WorkingDirectory = $target
    $link.IconLocation = "$exe,0"
    $link.Description = 'Tunify Music – Gesang in Instrumente und Stimmen umwandeln'
    $link.Save()
}
"Installiert: $exe"
$links | ForEach-Object { "Verknüpfung: $_" }
