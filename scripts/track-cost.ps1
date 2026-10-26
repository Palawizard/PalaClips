param(
    [int[]]$Counts = @(0, 6, 12, 24),
    [int]$Seconds = 45,
    [string]$Device = "Realtek",
    [string]$Name = "track-cost"
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
    (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$ProcessId" -ErrorAction SilentlyContinue).WorkingSetPrivate
}

$tones = @()
try {
    if (-not (Test-Path $cli)) { throw "compile d'abord : cargo build --release -p palaclips-cli" }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $work | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $most = ($Counts | Measure-Object -Maximum).Maximum
    $total = ($Counts.Count * ($Seconds + 15)) + 10
    foreach ($index in 1..$most) {
        $exe = Join-Path $work ("tone-{0:D2}.exe" -f $index)
        Copy-Item $cli $exe
        $tones += Start-Process $exe -ArgumentList "play-tone", "--hz", "$(200 + 37 * $index)", "--amplitude", "0.1", "--seconds", "$total", "--device", $Device -PassThru -WindowStyle Hidden
    }
    Start-Sleep -Seconds 3
    Write-Journal "$(Get-Date -Format s) $Name : $most sons de test vers « $Device », capture écran 480p30 1 Mb/s, $Seconds s par essai, $cores processeurs logiques"
    $baseline = $null
    foreach ($count in $Counts) {
        $out = Join-Path $work "capture-$count.mp4"
        $arguments = @("capture-apps", "--target", "primary", "--height", "480", "--fps", "30", "--bitrate-mbps", "1", "--no-mix", "--seconds", "$Seconds", "--out", $out)
        foreach ($index in 1..([math]::Max($count, 1))) {
            if ($count -gt 0) { $arguments += @("--app", ("tone-{0:D2}.exe" -f $index)) }
        }
        $process = Start-Process $cli -ArgumentList $arguments -PassThru -WindowStyle Hidden -RedirectStandardOutput "$out.log" -RedirectStandardError "$out.err"
        Start-Sleep -Seconds 5
        $cpuStart = $process.TotalProcessorTime
        $wallStart = Get-Date
        $memory = @()
        while (-not $process.HasExited -and ((Get-Date) - $wallStart).TotalSeconds -lt ($Seconds - 8)) {
            $sample = Get-PrivateBytes $process.Id
            if ($sample) { $memory += [double]$sample }
            Start-Sleep -Seconds 1
        }
        $process.Refresh()
        $cpu = 100 * ($process.TotalProcessorTime - $cpuStart).TotalSeconds / ((Get-Date) - $wallStart).TotalSeconds / $cores
        if (-not $process.WaitForExit(($Seconds + 30) * 1000)) { Stop-Process -Id $process.Id -Force; throw "capture de $count pistes bloquée" }
        if (-not (Test-Path $out)) { throw "capture de $count pistes : pas de fichier ($(Get-Content "$out.err" -Raw))" }
        $streams = @(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 $out).Count
        $audioBytes = [double](ffprobe -v error -select_streams a -show_entries packet=size -of csv=p=0 $out | Measure-Object -Sum).Sum
        $duration = [double](ffprobe -v error -show_entries format=duration -of csv=p=0 $out)
        $memoryMb = ($memory | Measure-Object -Average).Average / 1MB
        $row = [pscustomobject]@{ Count = $count; Cpu = $cpu; Memory = $memoryMb; AudioMbPerMin = $audioBytes / 1e6 / $duration * 60; Streams = $streams }
        if ($count -eq 0) { $baseline = $row }
        $perTrack = if ($baseline -and $count -gt 0) {
            " | par piste : CPU {0:N3} %, mémoire {1:N1} Mo, {2:N2} Mo/min" -f (($cpu - $baseline.Cpu) / $count), (($memoryMb - $baseline.Memory) / $count), ($row.AudioMbPerMin / $count)
        } else { "" }
        Write-Journal ("{0,2} pistes : {1} flux audio, CPU {2:N2} % de la machine, mémoire privée {3:N1} Mo, audio {4:N2} Mo/min{5}" -f $count, $streams, $cpu, $memoryMb, $row.AudioMbPerMin, $perTrack)
        Remove-Item $out -Force
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $tones | Where-Object { -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Get-Process palaclips-cli -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : sons de test arrêtés, fichiers supprimés, journal $journal"
}
