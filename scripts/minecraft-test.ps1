param(
    [string]$Exe = "javaw.exe",
    [int]$Seconds = 30,
    [string]$Name = "minecraft"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$work = Join-Path $root "target\$Name"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

$probe = $null
try {
    if (-not (Get-Process ([IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue)) {
        throw "$Exe n'est pas lancé"
    }
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $work | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $clip = Join-Path $work "clip.mp4"
    $probeOut = Join-Path $work "probe.txt"
    $probe = Start-Process $cli -ArgumentList @("probe-fullscreen", "--seconds", ($Seconds + 2)) -RedirectStandardOutput $probeOut -WindowStyle Hidden -PassThru
    Write-Journal "$(Get-Date -Format s) $Name : capture de window:$Exe pendant $Seconds s (720p, 60 i/s) et sonde du plein écran exclusif"
    $capture = & $cli capture-video --target "window:$Exe" --seconds $Seconds --height 720 --fps 60 --out $clip 2>&1 | ForEach-Object { "$_" -replace "`e\[[0-9;]*m", '' }
    $capture | Select-String "images|repli|WARN|ERROR|Error" | ForEach-Object { Write-Journal "capture : $($_.Line.Trim())" }
    $probe.WaitForExit(15000) | Out-Null
    $samples = @(Get-Content $probeOut)
    $exclusive = @($samples | Where-Object { $_ -match "exclusif \(D3DKMT\) true" }).Count
    $d3dFull = @($samples | Where-Object { $_ -match "RUNNING_D3D_FULL_SCREEN" }).Count
    $foreground = $samples | ForEach-Object { if ($_ -match "premier plan (\S+)") { $Matches[1] } } | Group-Object | ForEach-Object { "$($_.Name) $($_.Count) s" }
    Write-Journal "sonde : exclusif (D3DKMT) $exclusive s sur $($samples.Count), notifications RUNNING_D3D_FULL_SCREEN $d3dFull s, premier plan : $($foreground -join ', ')"
    if (-not (Test-Path $clip)) { throw "aucun fichier vidéo écrit" }
    $duration = [double](ffprobe -v error -show_entries format=duration -of csv=p=0 $clip)
    $events = ffmpeg -hide_banner -nostats -i $clip -map 0:v -vf "freezedetect=n=0.003:d=1" -f null - 2>&1 | Select-String "freeze_(start|end): ([0-9.]+)"
    $frozen = @()
    $start = $null
    foreach ($event in $events) {
        $kind = $event.Matches[0].Groups[1].Value
        $at = [double]$event.Matches[0].Groups[2].Value
        if ($kind -eq "start") { $start = $at } elseif ($null -ne $start) { $frozen += $at - $start; $start = $null }
    }
    if ($null -ne $start) { $frozen += $duration - $start }
    $frozenTotal = [math]::Round(($frozen | Measure-Object -Sum).Sum, 1)
    $received = $capture | Select-String "\((\d+) reçues de Windows" | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Select-Object -First 1
    Write-Journal "images reçues de Windows : $received en $Seconds s"
    foreach ($at in 5, 15, 25) {
        ffmpeg -hide_banner -loglevel error -y -ss $at -i $clip -frames:v 1 (Join-Path $work "frame-$at.jpg")
    }
    Write-Journal "vidéo : $([math]::Round($duration, 1)) s, image figée au total $frozenTotal s ($(@($frozen).Count) passage(s) d'au moins 1 s sans changement)"
    $alive = $frozenTotal -lt 3 -and $received -ge $Seconds * 5
    Write-Journal "RÉSULTAT : $(if ($alive) { "image vivante" } else { "image FIGÉE ($received image(s) reçue(s), $frozenTotal s sans changement sur $([math]::Round($duration, 1)) s)" }) ; plein écran exclusif détecté $exclusive s"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    if ($probe -and -not $probe.HasExited) { Stop-Process -Id $probe.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item (Join-Path $work "clip.mp4") -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : capture et sonde arrêtées, vidéo supprimée (3 images gardées dans $work pour vérification), journal $journal"
}
