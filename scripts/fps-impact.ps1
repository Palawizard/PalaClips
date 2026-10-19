param(
    [string]$Game = "FortniteClient-Win64-Shipping.exe",
    [string]$App = "vesktop.exe",
    [int]$Seconds = 20,
    [int]$BitrateMbps = 40,
    [string]$PresentMon = "C:\Program Files (x86)\CapFrameX\PresentMon\PresentMon-2.4.1-x64.exe",
    [string]$Name = "fps-impact"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$work = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir, $work | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$cli = Join-Path $root "target\release\palaclips-cli.exe"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Measure-Fps([string]$Label, [string[]]$CliArguments) {
    $csv = Join-Path $work "$($Label -replace '\W', '_').csv"
    $capture = $null
    if ($CliArguments) {
        $capture = Start-Process $cli -ArgumentList $CliArguments -PassThru -NoNewWindow -RedirectStandardError "$csv.log" -RedirectStandardOutput "$csv.out"
        Start-Sleep -Seconds 3
    }
    $gpuLog = "$csv.gpu"
    $gpu = Start-Process nvidia-smi -ArgumentList "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits", "-lms", "500" -PassThru -WindowStyle Hidden -RedirectStandardOutput $gpuLog -ErrorAction SilentlyContinue
    & $PresentMon --process_name $Game --timed $Seconds --terminate_after_timed --output_file $csv --no_console_stats --stop_existing_session --v1_metrics | Out-Null
    if ($gpu) { Stop-Process -Id $gpu.Id -Force -ErrorAction SilentlyContinue }
    $gpuSamples = @(Get-Content $gpuLog -ErrorAction SilentlyContinue | Where-Object { $_ -match '^\s*\d+\s*$' } | ForEach-Object { [double]$_ })
    $gpuBusy = if ($gpuSamples) { "GPU {0:N0} %" -f ($gpuSamples | Measure-Object -Average).Average } else { "GPU ?" }
    if ($capture) {
        if (-not $capture.WaitForExit(($Seconds + 30) * 1000)) { Stop-Process -Id $capture.Id -Force }
    }
    $rows = @(Import-Csv $csv | Where-Object { $_.MsBetweenPresents -and [double]$_.MsBetweenPresents -gt 0 })
    if ($rows.Count -lt 10) { throw "$Label : pas de présentations de $Game (le jeu tourne-t-il ?)" }
    $frameTimes = $rows | ForEach-Object { [double]$_.MsBetweenPresents } | Sort-Object
    $average = 1000 / ($frameTimes | Measure-Object -Average).Average
    $low = 1000 / ($frameTimes[[int]([math]::Floor($frameTimes.Count * 0.99))])
    $mode = ($rows | Group-Object PresentMode | Sort-Object Count -Descending | Select-Object -First 1).Name
    $stats = (Get-Content "$csv.log", "$csv.out" -ErrorAction SilentlyContinue | Select-String "arrêtée|images reçues" | ForEach-Object { $_.Line -replace "`e\[[0-9;]*m", '' -replace '^.*arrêtée ', '' }) -join " "
    Write-Journal ("{0,-30} {1,6:N1} i/s, 1 % bas {2,6:N1} i/s, {3}, {4} présentations, mode « {5} » {6}" -f $Label, $average, $low, $gpuBusy, $rows.Count, $mode, $stats)
    [pscustomobject]@{ Label = $Label; Fps = $average; Low = $low }
}

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "Lance ce script dans un PowerShell administrateur (PresentMon en a besoin)."
    exit 1
}
if (-not (Test-Path $cli)) { throw "compile d'abord : cargo build --release -p palaclips-cli" }
if (-not (Test-Path $PresentMon)) { throw "PresentMon introuvable : $PresentMon" }
if (-not (Get-Process ($Game -replace '\.exe$', '') -ErrorAction SilentlyContinue)) { throw "$Game ne tourne pas" }
$env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
$target = "window:$Game"
$out = Join-Path $work "capture.mp4"
$video = @("--bitrate-mbps", "$BitrateMbps", "--seconds", "$($Seconds + 6)", "--out", $out)
try {
    Write-Journal "$(Get-Date -Format s) $Name : $Game, $Seconds s par essai, PresentMon (MsBetweenPresents) ; ne pas bouger"
    $null = Measure-Fps "mise en route (non comptée)" @()
    $runs = @(
        @{ Label = "fenêtre 60 i/s"; Arguments = (@("capture-video", "--target", $target, "--fps", "60") + $video) }
        @{ Label = "écran principal 60 i/s"; Arguments = (@("capture-video", "--target", "primary", "--fps", "60") + $video) }
        @{ Label = "écran 1440p60 + 4 pistes"; Arguments = (@("capture-apps", "--target", "primary", "--fps", "60", "--mic", "--game", "--app", $App) + $video) }
        @{ Label = "fenêtre 30 i/s"; Arguments = (@("capture-video", "--target", $target, "--fps", "30") + $video) }
        @{ Label = "WGC seul (sans encodage)"; Arguments = @("probe-wgc", "--target", $target, "--seconds", "$($Seconds + 6)", "--min-interval-ms", "8") }
    )
    $before = Measure-Fps "sans capture 1" @()
    $losses = @()
    $index = 1
    foreach ($run in $runs) {
        $index += 1
        $capture = Measure-Fps $run.Label $run.Arguments
        $after = Measure-Fps "sans capture $index" @()
        $reference = ($before.Fps + $after.Fps) / 2
        $losses += [pscustomobject]@{ Label = $run.Label; Fps = $capture.Fps; Reference = $reference; Drift = 100 * ($before.Fps - $after.Fps) / $reference }
        $before = $after
    }
    foreach ($loss in $losses) {
        Write-Journal ("PERTE {0,-28} {1,6:N1} % ({2:+0.0;-0.0} i/s) face à la moyenne des essais sans capture voisins ({3:N1} i/s, dérive entre eux {4:N1} %)" -f $loss.Label, (100 * ($loss.Reference - $loss.Fps) / $loss.Reference), ($loss.Fps - $loss.Reference), $loss.Reference, $loss.Drift)
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    Get-Process palaclips-cli -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : captures arrêtées et supprimées, journal $journal"
}
