$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$sandbox = Join-Path $root "target\migration-check"
$realRoaming = Join-Path $env:APPDATA "PalaClips"
$realLocal = Join-Path $env:LOCALAPPDATA "PalaClips"
$cli = Join-Path $root "target\release\palaclips-cli.exe"
$env:PATH = "$(Join-Path $root "third_party\ffmpeg\bin");$env:PATH"
$dataEntries = '^\\(cache|replay)\\'

function Get-Hashes([string]$Dir) {
    if (-not (Test-Path $Dir)) { return @{} }
    $hashes = @{}
    foreach ($file in Get-ChildItem $Dir -Recurse -File -Force) {
        $hashes[$file.FullName.Substring($Dir.Length)] = (Get-FileHash $file.FullName -Algorithm SHA256).Hash
    }
    $hashes
}

function Select-Hashes([hashtable]$Hashes, [scriptblock]$Keep) {
    $selected = @{}
    foreach ($key in $Hashes.Keys) { if (& $Keep $key) { $selected[$key] = $Hashes[$key] } }
    $selected
}

function Compare-Hashes([hashtable]$Expected, [hashtable]$Actual, [string]$Label) {
    $problems = @()
    foreach ($key in $Expected.Keys) {
        if (-not $Actual.ContainsKey($key)) { $problems += "$Label : $key manquant" }
        elseif ($Actual[$key] -ne $Expected[$key]) { $problems += "$Label : $key différent" }
    }
    $problems
}

function Get-UninstallEntry {
    Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' |
        Where-Object { (Get-ItemProperty $_.PSPath).DisplayName -eq 'PalaClips' } |
        ForEach-Object { (Get-ItemProperty $_.PSPath | Select-Object DisplayName, DisplayVersion, InstallLocation, UninstallString | ConvertTo-Json -Compress) }
}

Remove-Item $sandbox -Recurse -Force -ErrorAction SilentlyContinue
try {
    $realRoamingBefore = Get-Hashes $realRoaming
    $realLocalBefore = Get-Hashes $realLocal
    $entryBefore = Get-UninstallEntry
    $localData = Select-Hashes $realLocalBefore { param($key) $key -match $dataEntries }
    $localOther = Select-Hashes $realLocalBefore { param($key) $key -notmatch $dataEntries }
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
    $problems += Compare-Hashes $realRoamingBefore (Get-Hashes $newRoaming) "Roaming migré"
    $problems += Compare-Hashes $localData (Get-Hashes $newLocal) "données Local migrées"
    $problems += Compare-Hashes $localOther (Get-Hashes $oldLocal) "fichiers hors données laissés en place"
    if (Test-Path $oldRoaming) { $problems += "ancien Roaming encore présent : $(@(Get-ChildItem $oldRoaming -Recurse -File).Count) fichier(s)" }
    if (-not (Test-Path $oldLocal)) { $problems += "ancien dossier Local supprimé" }
    foreach ($entry in 'cache', 'replay') { if (Test-Path (Join-Path $oldLocal $entry)) { $problems += "$entry encore dans l'ancien Local" } }
    $problems += Compare-Hashes $realRoamingBefore (Get-Hashes $realRoaming) "Roaming réel"
    $problems += Compare-Hashes $realLocalBefore (Get-Hashes $realLocal) "Local réel"
    if ((Get-UninstallEntry) -ne $entryBefore) { $problems += "entrée de désinstallation modifiée" }
    "copie de tes données : $($realRoamingBefore.Count) fichier(s) Roaming, $($localData.Count) fichier(s) de données Local migrés ; $($localOther.Count) fichier(s) hors données (programme installé, inconnus) laissés en place ; entrée de désinstallation : $(if ($entryBefore) { 'présente' } else { 'absente' })"
    if ($problems.Count -eq 0) { "OK" } else { "ÉCHEC : $($problems -join ' ; ')"; exit 1 }
}
finally {
    Remove-Item $sandbox -Recurse -Force -ErrorAction SilentlyContinue
}
