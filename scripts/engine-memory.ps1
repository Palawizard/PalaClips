param(
    [int]$BitrateMbps = 40,
    [string]$ToneDevice = "Realtek",
    [string]$Name = "engine-memory"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$clips = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir, $clips | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Get-Median([double[]]$Values) {
    $sorted = $Values | Sort-Object
    $sorted[[int](($sorted.Count - 1) / 2)]
}

function Measure-Stage([string]$Label, [string]$Arguments, [int]$Seconds, [int]$SettleSeconds, [bool]$Interactive) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $script:cli
    $info.Arguments = $Arguments
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $process = [System.Diagnostics.Process]::Start($info)
    $script:started += $process
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $private = @(); $working = @(); $privateWorking = @()
    for ($s = 1; $s -le $Seconds; $s++) {
        Start-Sleep -Seconds 1
        if ($process.HasExited) { break }
        $process.Refresh()
        if ($s -gt $SettleSeconds) {
            $private += $process.PrivateMemorySize64 / 1e6
            $working += $process.WorkingSet64 / 1e6
            $counters = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess=$($process.Id)" -ErrorAction SilentlyContinue
            if ($counters) { $privateWorking += $counters.WorkingSetPrivate / 1e6 }
        }
    }
    if ($Interactive -and -not $process.HasExited) { $process.StandardInput.WriteLine("q") }
    if (-not $process.WaitForExit(60000)) { throw "$Label : le processus ne s'est pas arrêté" }
    if ($private.Count -lt 3) { throw "$Label : arrêté trop tôt : $($stdout.Result.Trim()) $($stderr.Result.Trim())" }
    $buffer = 0.0
    foreach ($line in $stdout.Result -split "`r?`n") {
        if ($line -match 'tampon ([\d.]+) s, (\d+) Mo') { $buffer = [double]$Matches[2] }
    }
    $warnings = ($stderr.Result -split "`n" | Select-String "WARN|ERROR" | Out-String).Trim()
    if ($warnings) { Write-Journal "$Label avertissements : $warnings" }
    $result = [pscustomobject]@{
        Label = $Label
        Private = Get-Median $private
        Working = Get-Median $working
        PrivateWorking = Get-Median $privateWorking
        Buffer = $buffer
        Samples = $private.Count
    }
    Write-Journal ("{0,-28} privée {1,7:N1} Mo, working set {2,7:N1} Mo, working set privé {3,7:N1} Mo, tampon journalisé {4,6:N1} Mo ({5} mesures)" -f $result.Label, $result.Private, $result.Working, $result.PrivateWorking, $result.Buffer, $result.Samples)
    $result
}

Add-Type -AssemblyName System.Windows.Forms
$started = @()
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $toneA = Join-Path $root "target\tone-a.exe"
    $toneB = Join-Path $root "target\tone-b.exe"
    Copy-Item $cli $toneA -Force
    Copy-Item $cli $toneB -Force
    Write-Journal "$(Get-Date -Format s) $Name : 1440p60 NVENC $BitrateMbps Mb/s, 4 pistes (mix, tone-a, tone-b, Jeu), fenêtre de bruit ffplay ; médiane des mesures après mise en régime"
    $area = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $started += Start-Process (Join-Path $root "third_party\ffmpeg\bin\ffplay.exe") -ArgumentList @(
        "-hide_banner", "-loglevel", "error", "-an",
        "-x", "1280", "-y", "720", "-left", "$($area.Right - 1300)", "-top", "$($area.Bottom - 760)",
        "-f", "lavfi", "-i", "nullsrc=s=640x360:r=60,geq=random(1)*255:128:128"
    ) -PassThru
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Process -Id $started[0].Id).MainWindowHandle -eq 0) {
        if ((Get-Date) -gt $deadline) { throw "la fenêtre ffplay ne s'est pas ouverte" }
        Start-Sleep -Milliseconds 200
    }
    $started += Start-Process $toneA -ArgumentList "play-tone", "--hz", "440", "--seconds", "300", "--device", $ToneDevice -PassThru -NoNewWindow
    $started += Start-Process $toneB -ArgumentList "play-tone", "--hz", "880", "--seconds", "300", "--device", $ToneDevice -PassThru -NoNewWindow
    Start-Sleep -Seconds 2

    $video = "--target window:ffplay.exe --height 1440 --fps 60 --bitrate-mbps $BitrateMbps"
    $sources = "--no-mic --app tone-a.exe --app tone-b.exe"
    $a = Measure-Stage "(a) moteur sans capture" "replay $video $sources --no-arm --clips-dir `"$clips`"" 8 3 $true
    $b = Measure-Stage "(b) capture sans replay" "capture-apps $video --app tone-a.exe --app tone-b.exe --game --seconds 25 --out `"$clips\b.mp4`"" 25 12 $false
    $c = Measure-Stage "(c) replay 5 s" "replay $video $sources --duration 5 --clips-dir `"$clips`"" 30 17 $true
    $d = Measure-Stage "(d) replay 60 s" "replay $video $sources --duration 60 --clips-dir `"$clips`"" 95 80 $true

    $realBuffer = $d.Private - $c.Private
    $loggedBuffer = $d.Buffer - $c.Buffer
    Write-Journal ("buffer réel (d) − (c) : {0:N1} Mo ; tampon journalisé (d) − (c) : {1:N1} Mo ; écart {2:N1} Mo" -f $realBuffer, $loggedBuffer, ($realBuffer - $loggedBuffer))
    Write-Journal ("hors buffer (c) : {0:N1} Mo, dont tampon de (c) {1:N1} Mo → {2:N1} Mo ; capture seule (b) − (a) : {3:N1} Mo ; moteur à vide (a) : {4:N1} Mo" -f $c.Private, $c.Buffer, ($c.Private - $c.Buffer), ($b.Private - $a.Private), $a.Private)
    Write-Journal ("mémoire engagée (information) : hors buffer (c) {0:N1} Mo, {1:N1} Mo sans le tampon de (c)" -f $c.Private, ($c.Private - $c.Buffer))
    $engine = $c.PrivateWorking - $c.Buffer * 1.07
    Write-Journal ("RÉSULTAT PERF-02 (working set privé − tampon × 1,07, ADR-023, même calcul que le panneau de performances) : (c) {0:N1} Mo, {1:N1} Mo hors tampon (seuil 150) → {2}" -f $c.PrivateWorking, $engine, $(if ($engine -lt 150) { "OK" } else { "ÉCHEC" }))
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : ffplay, sons de test et moteur arrêtés, clips supprimés, journal $journal"
}
