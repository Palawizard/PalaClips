<#
.SYNOPSIS
    Télécharge une build FFmpeg LGPL partagée (DLL) pour Windows dans third_party/ffmpeg.

.DESCRIPTION
    Source : BtbN/FFmpeg-Builds (builds automatiques, licence LGPL, DLL).
    Le dossier obtenu contient :
      third_party/ffmpeg/include  -> FFMPEG_INCLUDE_DIR (voir .cargo/config.toml)
      third_party/ffmpeg/bin      -> FFMPEG_DLL_PATH (DLL + .lib copiés à côté)
    Les DLL doivent ensuite être livrées à côté de l'exécutable (voir docs/ARCHITECTURE.md).

.PARAMETER Version
    Branche FFmpeg BtbN, ex. "n8.1" (défaut). rsmpeg 0.18 cible FFmpeg 8.x.

.EXAMPLE
    pwsh scripts/fetch-ffmpeg.ps1
    pwsh scripts/fetch-ffmpeg.ps1 -Version n8.1 -Force
#>
[CmdletBinding()]
param(
    [string]$Version = "n8.1",
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "third_party/ffmpeg"

if ((Test-Path (Join-Path $dest "bin")) -and -not $Force) {
    Write-Host "FFmpeg déjà présent dans $dest (utilise -Force pour retélécharger)." -ForegroundColor Yellow
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

Write-Host "Extraction..."
Expand-Archive -Path $zip -DestinationPath $tmp -Force
$extracted = Get-ChildItem -Path $tmp -Directory | Select-Object -First 1
if (-not $extracted) { throw "Archive inattendue : aucun dossier trouvé dans $asset" }

if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
New-Item -ItemType Directory -Path (Split-Path -Parent $dest) -Force | Out-Null
Move-Item -Path $extracted.FullName -Destination $dest

# rusty_ffmpeg (liaison dynamique) attend les .lib à côté des .dll.
Copy-Item -Path (Join-Path $dest "lib/*.lib") -Destination (Join-Path $dest "bin") -Force

Remove-Item -Recurse -Force $tmp

Write-Host ""
Write-Host "FFmpeg $Version prêt dans $dest" -ForegroundColor Green
Write-Host "Les variables FFMPEG_INCLUDE_DIR et FFMPEG_DLL_PATH sont fournies par .cargo/config.toml."
Write-Host "Rappel : bindgen a besoin de LLVM (winget install LLVM.LLVM) et éventuellement de LIBCLANG_PATH."
