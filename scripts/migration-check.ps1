$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$sandbox = Join-Path $root "target\migration-check"
$realRoaming = Join-Path $env:APPDATA "PalaClips"
$realLocal = Join-Path $env:LOCALAPPDATA "PalaClips"
$cli = Join-Path $root "target\release\palaclips-cli.exe"
$env:PATH = "$(Join-Path $root "third_party\ffmpeg\bin");$env:PATH"

function Get-Hashes([string]$Dir) {
    if (-not (Test-Path $Dir)) { return @{} }
    $hashes = @{}
    foreach ($file in Get-ChildItem $Dir -Recurse -File) {
        $hashes[$file.FullName.Substring($Dir.Length)] = (Get-FileHash $file.FullName -Algorithm SHA256).Hash
    }
    $hashes
}

function Compare-Hashes([hashtable]$Expected, [hashtable]$Actual, [string]$Label) {
    $problems = @()
    foreach ($key in $Expected.Keys) {
        if (-not $Actual.ContainsKey($key)) { $problems += "$Label : $key manquant" }
        elseif ($Actual[$key] -ne $Expected[$key]) { $problems += "$Label : $key différent" }
    }
    $problems
}

Remove-Item $sandbox -Recurse -Force -ErrorAction SilentlyContinue
try {
    $realRoamingBefore = Get-Hashes $realRoaming
    $realLocalBefore = Get-Hashes $realLocal
    $oldRoaming = Join-Path $sandbox "Roaming\PalaClips"
    $oldLocal = Join-Path $sandbox "Local\PalaClips"
    New-Item -ItemType Directory -Force $oldRoaming, $oldLocal | Out-Null
    if (Test-Path $realRoaming) { Copy-Item "$realRoaming\*" $oldRoaming -Recurse -Force }
    if (Test-Path $realLocal) { Copy-Item "$realLocal\*" $oldLocal -Recurse -Force }
    $newRoaming = Join-Path $sandbox "Roaming\com.palawi.palaclips"
    $newLocal = Join-Path $sandbox "Local\com.palawi.palaclips"
    & $cli migrate-data --old-roaming $oldRoaming --old-local $oldLocal --new-roaming $newRoaming --new-local $newLocal
    if ($LASTEXITCODE -ne 0) { throw "migrate-data a échoué" }
    $problems = @()
    $problems += Compare-Hashes $realRoamingBefore (Get-Hashes $newRoaming) "Roaming"
    $problems += Compare-Hashes $realLocalBefore (Get-Hashes $newLocal) "Local"
    if (Test-Path $oldRoaming) { $problems += "ancien Roaming encore présent : $(@(Get-ChildItem $oldRoaming -Recurse -File).Count) fichier(s)" }
    if (Test-Path $oldLocal) { $problems += "ancien Local encore présent : $(@(Get-ChildItem $oldLocal -Recurse -File).Count) fichier(s)" }
    $problems += Compare-Hashes $realRoamingBefore (Get-Hashes $realRoaming) "Roaming réel"
    $problems += Compare-Hashes $realLocalBefore (Get-Hashes $realLocal) "Local réel"
    "copie de tes données : $($realRoamingBefore.Count) fichier(s) Roaming, $($realLocalBefore.Count) fichier(s) Local ; toutes retrouvées identiques (SHA-256) au nouvel emplacement : $($problems.Count -eq 0)"
    if ($problems.Count -eq 0) { "OK" } else { "ÉCHEC : $($problems -join ' ; ')"; exit 1 }
}
finally {
    Remove-Item $sandbox -Recurse -Force -ErrorAction SilentlyContinue
}
