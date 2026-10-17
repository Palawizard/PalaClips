param([string]$Settings = '{}')
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (Get-Process palaclips-app -ErrorAction SilentlyContinue) {
    throw "PalaClips est déjà ouvert : l'instance unique enverrait le test vers lui. Ferme-le d'abord."
}
$dir = Join-Path $root 'target\test-config'
if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
New-Item -ItemType Directory -Force (Join-Path $dir 'clips') | Out-Null
$json = $Settings | ConvertFrom-Json -AsHashtable
if (-not $json.engine) { $json.engine = @{} }
if (-not $json.engine.storage) { $json.engine.storage = @{} }
$json.engine.storage.clips_dir = Join-Path $dir 'clips'
$json | ConvertTo-Json -Depth 20 | Set-Content (Join-Path $dir 'settings.json') -Encoding utf8
$env:PALACLIPS_CONFIG_DIR = $dir
$env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
Start-Process (Join-Path $root 'target\release\palaclips-app.exe') -PassThru
