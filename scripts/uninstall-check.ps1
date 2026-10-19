param(
    [Parameter(Mandatory)][ValidateSet("Backup", "Verify")][string]$Step,
    [string]$Roaming = (Join-Path $env:APPDATA "com.palawi.palaclips"),
    [string]$Local = (Join-Path $env:LOCALAPPDATA "com.palawi.palaclips"),
    [string]$Backup = "",
    [string]$Clips = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$backupDir = if ($Backup) { $Backup } else { Join-Path $root "target\uninstall-backup" }
$dirs = [ordered]@{ roaming = $Roaming; local = $Local }

function Get-ClipsDir {
    if ($Clips) { return $Clips }
    foreach ($settings in @((Join-Path $dirs.roaming "settings.json"), (Join-Path $env:APPDATA "PalaClips\settings.json"))) {
        if (Test-Path $settings) {
            $dir = (Get-Content $settings -Raw | ConvertFrom-Json).engine.storage.clips_dir
            if ($dir) { return $dir }
        }
    }
    Join-Path ([Environment]::GetFolderPath("MyVideos")) "PalaClips"
}

function Get-Hashes([string]$Dir) {
    $hashes = [ordered]@{}
    if (Test-Path $Dir) {
        foreach ($file in Get-ChildItem $Dir -Recurse -File -Force | Sort-Object FullName) {
            $hashes[$file.FullName.Substring($Dir.Length)] = (Get-FileHash $file.FullName -Algorithm SHA256).Hash
        }
    }
    $hashes
}

function Compare-Hashes($Expected, $Actual) {
    $problems = @()
    foreach ($key in $Expected.Keys) {
        if (-not $Actual.Contains($key)) { $problems += "$key manquant" }
        elseif ($Actual[$key] -ne $Expected[$key]) { $problems += "$key différent" }
    }
    foreach ($key in $Actual.Keys) { if (-not $Expected.Contains($key)) { $problems += "$key en trop" } }
    $problems
}

$running = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in @('palaclips-app', 'PalaClips') }
if ($running) { throw "PalaClips tourne : quitte-le (zone de notification > Quitter) avant cette étape" }

if ($Step -eq "Backup") {
    if (Test-Path $backupDir) { throw "une sauvegarde existe déjà dans $backupDir : lance d'abord -Step Verify" }
    $clipsDir = Get-ClipsDir
    $manifest = [ordered]@{ clips_dir = $clipsDir; clips = Get-Hashes $clipsDir }
    New-Item -ItemType Directory -Force $backupDir | Out-Null
    foreach ($name in $dirs.Keys) {
        $manifest[$name] = Get-Hashes $dirs[$name]
        if (Test-Path $dirs[$name]) { Copy-Item $dirs[$name] (Join-Path $backupDir $name) -Recurse -Force }
    }
    $manifest | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $backupDir "manifest.json") -Encoding utf8
    foreach ($name in $dirs.Keys) {
        $problems = Compare-Hashes $manifest[$name] (Get-Hashes (Join-Path $backupDir $name))
        if ($problems) { throw "copie de $name incomplète : $($problems -join ' ; ')" }
    }
    "sauvegarde vérifiée : $($manifest.roaming.Count) fichier(s) Roaming, $($manifest.local.Count) fichier(s) Local ; dossier des clips $clipsDir : $($manifest.clips.Count) fichier(s)"
    exit 0
}

if (-not (Test-Path (Join-Path $backupDir "manifest.json"))) { throw "aucune sauvegarde dans $backupDir : lance d'abord -Step Backup" }
$manifest = Get-Content (Join-Path $backupDir "manifest.json") -Raw | ConvertFrom-Json -AsHashtable
foreach ($name in $dirs.Keys) {
    "$name après désinstallation : $(if (Test-Path $dirs[$name]) { "encore présent ($(@(Get-ChildItem $dirs[$name] -Recurse -File -Force).Count) fichier(s))" } else { 'supprimé' })"
}
$clipsProblems = Compare-Hashes $manifest.clips (Get-Hashes $manifest.clips_dir)
"clips ($($manifest.clips_dir)) : $(if ($clipsProblems) { "MODIFIÉS : $($clipsProblems -join ' ; ')" } else { "$($manifest.clips.Count) fichier(s) intacts" })"
$restoreProblems = @()
foreach ($name in $dirs.Keys) {
    $source = Join-Path $backupDir $name
    if (-not (Test-Path $source)) { continue }
    Remove-Item $dirs[$name] -Recurse -Force -ErrorAction SilentlyContinue
    Copy-Item $source $dirs[$name] -Recurse -Force
    $restoreProblems += @(Compare-Hashes $manifest[$name] (Get-Hashes $dirs[$name]) | ForEach-Object { "$name : $_" })
}
"restauration : $(if ($restoreProblems) { "ÉCARTS : $($restoreProblems -join ' ; ')" } else { 'identique en SHA-256' })"
if ($restoreProblems -or $clipsProblems) {
    "sauvegarde gardée dans $backupDir"
    exit 1
}
Remove-Item $backupDir -Recurse -Force
"sauvegarde supprimée"
