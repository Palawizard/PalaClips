param(
    [int]$Seconds = 20,
    [string]$Name = "dual-encoder"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$cli = Join-Path $root "target\release\palaclips-cli.exe"
$work = Join-Path $root "target\$Name"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$cores = [Environment]::ProcessorCount

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Get-PrivateBytes([int]$ProcessId) {
    [double](Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$ProcessId" -ErrorAction SilentlyContinue).WorkingSetPrivate
}

function Measure-Run([string]$Label, [string[]]$Extra) {
    $out = Join-Path $work "a.mp4"
    $also = Join-Path $work "b.mp4"
    $arguments = @("capture-video", "--target", "primary", "--fps", "60", "--bitrate-mbps", "40", "--seconds", "$Seconds", "--out", $out) + ($Extra | ForEach-Object { $_ -replace '\{also\}', $also })
    $gpuLog = Join-Path $work "gpu.csv"
    $process = Start-Process $cli -ArgumentList $arguments -PassThru -WindowStyle Hidden -RedirectStandardOutput "$out.log" -RedirectStandardError "$out.err"
    Start-Sleep -Seconds 4
    $gpu = Start-Process nvidia-smi -ArgumentList "--query-gpu=utilization.gpu,utilization.encoder,memory.used,encoder.stats.sessionCount,encoder.stats.averageLatency", "--format=csv,noheader,nounits", "-lms", "500" -PassThru -WindowStyle Hidden -RedirectStandardOutput $gpuLog
    $cpuStart = $process.TotalProcessorTime
    $wallStart = Get-Date
    $memory = @()
    while (-not $process.HasExited -and ((Get-Date) - $wallStart).TotalSeconds -lt ($Seconds - 8)) {
        $memory += Get-PrivateBytes $process.Id
        Start-Sleep -Seconds 1
    }
    $process.Refresh()
    $cpu = 100 * ($process.TotalProcessorTime - $cpuStart).TotalSeconds / ((Get-Date) - $wallStart).TotalSeconds / $cores
    Stop-Process -Id $gpu.Id -Force -ErrorAction SilentlyContinue
    if (-not $process.WaitForExit(($Seconds + 30) * 1000)) { Stop-Process -Id $process.Id -Force; throw "$Label bloqué" }
    $rows = @(Get-Content $gpuLog | Where-Object { $_ -match '^[\d\s,]+$' } | ForEach-Object { , ($_ -split ',' | ForEach-Object { [double]$_.Trim() }) })
    $avg = { param($i) ($rows | ForEach-Object { $_[$i] } | Measure-Object -Average).Average }
    $frames = (Get-Content "$out.log" -Raw) -replace "`e\[[0-9;]*m", ''
    $summary = (($frames -split "`n") | Where-Object { $_ -match "images" } | ForEach-Object { $_.Trim() -replace ' → .*$', '' }) -join " | "
    Write-Journal ("{0,-34} CPU {1:N2} %, mémoire privée {2:N0} Mo, GPU {3:N0} %, NVENC {4:N0} %, VRAM {5:N0} Mio, sessions {6:N0}, latence NVENC {7:N0} µs | {8}" -f $Label, $cpu, (($memory | Measure-Object -Average).Average / 1MB), (& $avg 0), (& $avg 1), (& $avg 2), (& $avg 3), (& $avg 4), $summary)
    Remove-Item $out, $also -Force -ErrorAction SilentlyContinue
}

try {
    if (-not (Test-Path $cli)) { throw "compile d'abord : cargo build --release -p palaclips-cli" }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $work | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    Write-Journal "$(Get-Date -Format s) $Name : écran principal natif 60 i/s 40 Mb/s, $Seconds s par essai, bureau (sans jeu)"
    Measure-Run "un encodage" @()
    Measure-Run "+ 2e 1080p30 20 Mb/s" @("--also-out", "{also}", "--also-height", "1080", "--also-fps", "30", "--also-bitrate-mbps", "20")
    Measure-Run "+ 2e natif 60 i/s 60 Mb/s" @("--also-out", "{also}", "--also-bitrate-mbps", "60")
    Measure-Run "+ 2e 1080p60 HEVC 20 Mb/s" @("--also-out", "{also}", "--also-height", "1080", "--also-codec", "hevc", "--also-bitrate-mbps", "20")
    Measure-Run "+ 2e 1080p30 CPU H.264" @("--also-out", "{also}", "--also-height", "1080", "--also-fps", "30", "--also-encoder", "cpu", "--also-bitrate-mbps", "12")
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    Get-Process palaclips-cli -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : captures arrêtées et supprimées, journal $journal"
}
