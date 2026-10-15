[CmdletBinding()]
param(
    [string]$Version = "n8.1",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "third_party/ffmpeg"

if ((Test-Path (Join-Path $dest "bin")) -and -not $Force) {
    Write-Host "FFmpeg déjà présent dans $dest (-Force pour retélécharger)." -ForegroundColor Yellow
    exit 0
}

$short = $Version.TrimStart("n")
$asset = "ffmpeg-$Version-latest-win64-lgpl-shared-$short.zip"
$url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/$asset"

$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("palaclips-ffmpeg-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
$zip = Join-Path $tmp $asset

Write-Host "Téléchargement de $url"
Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing

Expand-Archive -Path $zip -DestinationPath $tmp -Force
$extracted = Get-ChildItem -Path $tmp -Directory | Select-Object -First 1
if (-not $extracted) { throw "Archive inattendue : aucun dossier dans $asset" }

if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
New-Item -ItemType Directory -Path (Split-Path -Parent $dest) -Force | Out-Null
Move-Item -Path $extracted.FullName -Destination $dest
Copy-Item -Path (Join-Path $dest "lib/*.lib") -Destination (Join-Path $dest "bin") -Force
Remove-Item -Recurse -Force $tmp

Write-Host "FFmpeg $Version prêt dans $dest" -ForegroundColor Green
