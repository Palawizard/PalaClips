param([int]$Seconds = 60, [int]$Port = 9346, [string]$Exe = (Join-Path $env:LOCALAPPDATA "PalaClips\palaclips-app.exe"))

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exe = $Exe
if ($exe -like "$root\target\*") { $env:PATH = "$(Join-Path $root "third_party\ffmpeg\bin");$env:PATH" }
$config = Join-Path $root "target\perf-config"
$cdp = Join-Path $PSScriptRoot "cdp.mjs"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "perf-installed-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
function Write-Journal([string]$Line) { $Line | Tee-Object -FilePath $journal -Append | Write-Host }
function Page([string]$Expression) {
    $output = node $cdp $Port $Expression
    if ($LASTEXITCODE -ne 0) { throw "CDP : $output" }
    $output | ConvertFrom-Json
}

function Get-Tree([int]$ProcessId) {
    $all = Get-CimInstance Win32_Process
    $ids = [System.Collections.Generic.List[int]]::new()
    $ids.Add($ProcessId)
    for ($i = 0; $i -lt $ids.Count; $i++) {
        $all | Where-Object { $_.ParentProcessId -eq $ids[$i] } | ForEach-Object { $ids.Add([int]$_.ProcessId) }
    }
    $ids
}

function Measure-Idle([int]$ProcessId, [int]$Duration) {
    $ids = Get-Tree $ProcessId
    $cores = [Environment]::ProcessorCount
    $cpuStart = 0.0
    foreach ($id in $ids) { $cpuStart += (Get-Process -Id $id).TotalProcessorTime.TotalSeconds }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $memory = @()
    while ($clock.Elapsed.TotalSeconds -lt $Duration) {
        Start-Sleep -Seconds 1
        $memory += (Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$ProcessId").WorkingSetPrivate
    }
    $cpuEnd = 0.0
    foreach ($id in $ids) { $process = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($process) { $cpuEnd += $process.TotalProcessorTime.TotalSeconds } }
    $sorted = $memory | Sort-Object
    [pscustomobject]@{
        Processes = $ids.Count
        PrivateMb = $sorted[[int]($sorted.Count / 2)] / 1e6
        CpuPercent = 100 * ($cpuEnd - $cpuStart) / $clock.Elapsed.TotalSeconds / $cores
    }
}

function Close-Window([int]$ProcessId) {
    $process = Get-Process -Id $ProcessId
    if ($process.MainWindowHandle -ne 0) { $process.CloseMainWindow() | Out-Null }
    $deadline = (Get-Date).AddSeconds(15)
    while (@(Get-Tree $ProcessId).Count -gt 1 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 300 }
}

function Open-Window {
    $start = [DateTimeOffset]::Now.ToUnixTimeMilliseconds()
    Start-Process $exe | Out-Null
    $deadline = (Get-Date).AddSeconds(20)
    while ($true) {
        try {
            $paint = Page 'new Promise((resolve) => { const check = () => { const entry = performance.getEntriesByName("first-contentful-paint")[0]; const ready = document.querySelector("header [role=status]")?.textContent; if (entry && ready) resolve(performance.timeOrigin + entry.startTime); else setTimeout(check, 20); }; check(); })'
            return $paint - $start
        }
        catch {
            if ((Get-Date) -gt $deadline) { throw "la fenêtre ne s'est pas ouverte : $_" }
            Start-Sleep -Milliseconds 100
        }
    }
}

$app = $null
$failures = @()
try {
    if (-not (Test-Path $exe)) { throw "PalaClips n'est pas installé ($exe)" }
    $running = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in @('palaclips-app', 'PalaClips') }
    if ($running) { throw "PalaClips tourne déjà ($(($running | ForEach-Object Path) -join ', ')) : ferme-le d'abord" }
    Remove-Item $config -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force (Join-Path $config "clips") | Out-Null
    @{
        engine = @{
            replay = @{ arm_on_startup = $false; duration_secs = 30 }
            storage = @{ clips_dir = (Join-Path $config "clips") }
        }
    } | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $config "settings.json") -Encoding utf8
    Add-Type -AssemblyName System.Windows.Forms
    $primary = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $env:PALACLIPS_CONFIG_DIR = $config
    $env:PALACLIPS_TEST_WINDOW = "$($primary.X + 40),$($primary.Y + 40)"
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    Write-Journal "$(Get-Date -Format s) build installée $exe ($((Get-Item $exe).VersionInfo.FileVersion)), réglages isolés dans $config, $([Environment]::ProcessorCount) cœurs logiques"

    $app = Start-Process $exe -ArgumentList "--autostart" -PassThru
    Start-Sleep -Seconds 15
    $idle = Measure-Idle $app.Id $Seconds
    Write-Journal ("PERF-01 zone de notification, replay désarmé : {0:N1} Mo (working set privé, médiane), CPU {1:N2} % de la machine, {2} processus (seuils 30 Mo, ~0 %)" -f $idle.PrivateMb, $idle.CpuPercent, $idle.Processes)
    if ($idle.PrivateMb -ge 30) { $failures += "PERF-01 RAM $([math]::Round($idle.PrivateMb, 1)) Mo" }
    if ($idle.Processes -gt 1) { $failures += "PERF-01 : WebView encore présent" }

    $cold = Open-Window
    Close-Window $app.Id
    Start-Sleep -Seconds 5
    $warm = @(1..3 | ForEach-Object { $value = Open-Window; Close-Window $app.Id; Start-Sleep -Seconds 3; $value })
    $warmSorted = $warm | Sort-Object
    Write-Journal ("PERF-04 ouverture de la fenêtre jusqu'au premier rendu avec l'état : première {0:N0} ms, à chaud {1} ms (médiane {2:N0} ms, seuil 1000)" -f $cold, (($warm | ForEach-Object { [math]::Round($_) }) -join ", "), $warmSorted[1])
    if ($warmSorted[1] -ge 1000) { $failures += "PERF-04 $([math]::Round($warmSorted[1])) ms" }

    Open-Window | Out-Null
    Page 'window.__TAURI_INTERNALS__.invoke("replay_arm")' | Out-Null
    Start-Sleep -Seconds 45
    $status = Page 'window.__TAURI_INTERNALS__.invoke("get_state")'
    $buffer = [double]$status.buffer_bytes
    Close-Window $app.Id
    Start-Sleep -Seconds 15
    $armed = Measure-Idle $app.Id $Seconds
    $engine = $armed.PrivateMb - $buffer * 1.07 / 1e6
    $capture = $status.capture
    Write-Journal ("PERF-02 replay armé, fenêtre fermée : {0:N1} Mo privés, tampon {1:N1} Mo ({2:N1} s) → moteur hors tampon {3:N1} Mo (seuil 150) ; capture {4}x{5} à {6} i/s, {7} pistes" -f $armed.PrivateMb, ($buffer / 1e6), $status.buffered_secs, $engine, $capture.width, $capture.height, $capture.fps, @($status.tracks).Count)
    Write-Journal ("PERF-03 (part CPU) replay armé, fenêtre fermée : CPU {0:N2} % de la machine (seuil 2 %)" -f $armed.CpuPercent)
    if ($engine -ge 150) { $failures += "PERF-02 $([math]::Round($engine, 1)) Mo" }
    if ($armed.CpuPercent -ge 2) { $failures += "PERF-03 CPU $([math]::Round($armed.CpuPercent, 2)) %" }
}
catch {
    $failures += "$_"
}
finally {
    if ($app) { taskkill /F /T /PID $app.Id 2>&1 | Out-Null }
    Start-Sleep -Seconds 2
    Remove-Item $config -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW, Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    if ($failures.Count -eq 0) { Write-Journal "OK" } else { Write-Journal "ÉCHEC : $($failures -join ' ; ')" }
    Write-Host "journal : $journal"
}
if ($failures.Count -ne 0) { exit 1 }
