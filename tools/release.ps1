# Neue Version von Tunify Music veröffentlichen (GitHub Release + Update für installierte Apps).
#
#   1. Version in app/src-tauri/tauri.conf.json, app/package.json und app/src-tauri/Cargo.toml erhöhen, committen
#   2. powershell -ExecutionPolicy Bypass -File tools\release.ps1 -Notes "Was ist neu"
#
# Braucht: gh (angemeldet), den Signaturschlüssel %USERPROFILE%\.tauri\tunify-music.key (gut sichern – ohne ihn
# bekommen installierte Apps keine Updates mehr) und models\bundled-voices (mitgelieferte Stimmen).
# Installierte Apps lesen latest.json aus dem neuesten Release und installieren nur korrekt signierte Updates.

param(
    [Parameter(Mandatory)][string]$Notes,
    [string]$Key = (Join-Path $env:USERPROFILE '.tauri\tunify-music.key'),
    [switch]$Draft
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$app = Join-Path $root 'app'
$repo = 'tunahanakguen/TunifyMusic'

$version = (Get-Content (Join-Path $app 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
$tag = "v$version"
Write-Host "Version $version"

if (git -C $root status --porcelain) { throw 'Es gibt nicht committete Änderungen – erst committen.' }
if (git -C $root tag --list $tag) { throw "Tag $tag gibt es schon – Version erhöhen." }
if (-not (Test-Path $Key)) { throw "Signaturschlüssel fehlt: $Key" }
if (-not (Test-Path (Join-Path $root 'models\bundled-voices'))) { throw 'models\bundled-voices fehlt (mitgelieferte Stimmen).' }

# Bauen und signieren (der Schlüssel hat kein Passwort)
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $Key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''
Push-Location $app
try {
    npx tauri build
    if ($LASTEXITCODE -ne 0) { throw 'Bauen fehlgeschlagen' }
} finally {
    Pop-Location
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
}

$bundle = Join-Path $app "src-tauri\target\release\bundle\nsis"
$setup = Join-Path $bundle "Tunify Music_${version}_x64-setup.exe"
if (-not (Test-Path "$setup.sig")) { throw "Signatur fehlt: $setup.sig" }

# GitHub macht aus Leerzeichen Punkte – darum ein Name ohne Leerzeichen
$dist = Join-Path $root "dist\$tag"
New-Item -ItemType Directory -Force $dist | Out-Null
$asset = "TunifyMusic_${version}_x64-setup.exe"
Copy-Item $setup (Join-Path $dist $asset) -Force
Copy-Item "$setup.sig" (Join-Path $dist "$asset.sig") -Force

$latest = [ordered]@{
    version   = $version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
        'windows-x86_64' = [ordered]@{
            signature = (Get-Content "$setup.sig" -Raw).Trim()
            url       = "https://github.com/$repo/releases/download/$tag/$asset"
        }
    }
}
$json = $latest | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText((Join-Path $dist 'latest.json'), $json, (New-Object Text.UTF8Encoding $false))

git -C $root tag -a $tag -m "Tunify Music $version"
git -C $root push origin $tag
$flags = @('--title', "Tunify Music $version", '--notes', $Notes)
if ($Draft) { $flags += '--draft' }
gh release create $tag --repo $repo @flags (Join-Path $dist $asset) (Join-Path $dist "$asset.sig") (Join-Path $dist 'latest.json')
if ($LASTEXITCODE -ne 0) { throw 'GitHub-Release fehlgeschlagen' }
Write-Host "Veröffentlicht: https://github.com/$repo/releases/tag/$tag"
