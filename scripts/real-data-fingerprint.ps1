param([Parameter(Mandatory)][string]$Out, [string]$Compare = "")

$ErrorActionPreference = "Stop"
$folders = [ordered]@{
    "Roaming\PalaClips" = Join-Path $env:APPDATA "PalaClips"
    "Local\PalaClips\cache" = Join-Path $env:LOCALAPPDATA "PalaClips\cache"
    "Local\PalaClips\replay" = Join-Path $env:LOCALAPPDATA "PalaClips\replay"
    "Roaming\com.palawi.palaclips" = Join-Path $env:APPDATA "com.palawi.palaclips"
    "Local\com.palawi.palaclips" = Join-Path $env:LOCALAPPDATA "com.palawi.palaclips"
}

$fingerprint = [ordered]@{}
foreach ($label in $folders.Keys) {
    $dir = $folders[$label]
    if (-not (Test-Path $dir)) { continue }
    foreach ($file in Get-ChildItem $dir -Recurse -File -Force | Sort-Object FullName) {
        $fingerprint["$label$($file.FullName.Substring($dir.Length))"] = "$($file.Length) $((Get-FileHash $file.FullName -Algorithm SHA256).Hash)"
    }
}
$fingerprint | ConvertTo-Json | Set-Content $Out -Encoding utf8
"$($fingerprint.Count) fichier(s) relevés dans $(@($folders.Keys | Where-Object { Test-Path $folders[$_] }).Count) dossier(s) → $Out"

if ($Compare) {
    $before = Get-Content $Compare -Raw | ConvertFrom-Json -AsHashtable
    $changes = @()
    foreach ($key in $before.Keys) {
        if (-not $fingerprint.Contains($key)) { $changes += "supprimé : $key" }
        elseif ($fingerprint[$key] -ne $before[$key]) { $changes += "modifié : $key" }
    }
    foreach ($key in $fingerprint.Keys) { if (-not $before.ContainsKey($key)) { $changes += "ajouté : $key" } }
    if ($changes) { "ÉCARTS ($($changes.Count)) :"; $changes; exit 1 }
    "identique à $Compare"
}
