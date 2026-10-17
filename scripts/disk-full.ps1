param(
    [int]$SizeMb = 1024,
    [int]$BitrateMbps = 80,
    [int]$TimeoutSeconds = 240
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { throw "Monter un disque virtuel demande les droits administrateur : relance ce script dans un terminal administrateur." }

$freeC = (Get-PSDrive C).Free
if ($freeC -lt ($SizeMb * 1MB + 5GB)) { throw "Moins de $([math]::Round(($SizeMb * 1MB + 5GB) / 1GB)) Go libres sur C: : test annulé." }

$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "disk-full-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
function Write-Journal([string]$Line) { $Line | Tee-Object -FilePath $journal -Append | Write-Host }

$vhdx = Join-Path $root "target\disk-full.vhdx"
$config = Join-Path $root "target\test-config-disk-full"
$used = (Get-PSDrive -PSProvider FileSystem).Name
$letter = [char[]]([char]'Z'..[char]'K') | Where-Object { $used -notcontains [string]$_ } | Select-Object -First 1
if (-not $letter) { throw "aucune lettre de lecteur libre" }
$started = @()
$attached = $false

function Invoke-DiskPart([string[]]$Lines) {
    $script = Join-Path $env:TEMP "palaclips-diskpart.txt"
    $Lines | Set-Content $script -Encoding ascii
    $output = diskpart /s $script
    Remove-Item $script -Force
    if ($LASTEXITCODE -ne 0) { throw "diskpart : $($output -join ' ')" }
}

try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"

    Remove-Item $vhdx -Force -ErrorAction SilentlyContinue
    Invoke-DiskPart @(
        "create vdisk file=`"$vhdx`" maximum=$SizeMb type=expandable",
        "select vdisk file=`"$vhdx`"",
        "attach vdisk",
        "create partition primary",
        "format fs=ntfs quick label=PCFULL",
        "assign letter=$letter"
    )
    $attached = $true
    $clips = "${letter}:\clips"
    Remove-Item $config -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $config | Out-Null
    $env:PALACLIPS_CONFIG_DIR = $config
    Write-Journal "$(Get-Date -Format s) disque virtuel $SizeMb Mo monté en ${letter}: ; enregistrement à $BitrateMbps Mb/s d'une fenêtre de bruit jusqu'au disque plein"

    Add-Type -AssemblyName System.Windows.Forms
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

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = "replay --target window:ffplay.exe --height 1440 --fps 60 --bitrate-mbps $BitrateMbps --duration 10 --no-mic --no-mix --no-game --clips-dir `"$clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $replay = [System.Diagnostics.Process]::Start($info)
    $started += $replay
    $stdout = $replay.StandardOutput.ReadToEndAsync()
    $stderr = $replay.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 5
    if ($replay.HasExited) { throw "replay non armé : $($stdout.Result.Trim()) $($stderr.Result.Trim())" }
    $replay.StandardInput.WriteLine("e")
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-PSDrive $letter).Free -gt 320MB -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 2 }
    Start-Sleep -Seconds 10
    $replay.StandardInput.WriteLine("")
    Start-Sleep -Seconds 3
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(60000)) { throw "le replay ne s'est pas arrêté" }
    Write-Journal $stdout.Result.TrimEnd()
    Write-Journal ($stderr.Result -split "`n" | Select-String "WARN|ERROR" | Out-String).TrimEnd()

    $failures = @()
    if ($stdout.Result -notmatch "alerte Error recording_failed") { $failures += "pas d'alerte recording_failed" }
    if ($stdout.Result -notmatch "alerte Warning disk_almost_full") { $failures += "pas d'alerte disk_almost_full" }
    if ($stdout.Result -notmatch "alerte Warning clip_save_failed") { $failures += "sauvegarde du replay non refusée" }
    $files = @(Get-ChildItem $clips -Recurse -Filter *.mp4)
    if ($files.Count -lt 1) { $failures += "aucun fichier" }
    foreach ($file in $files) {
        $probe = & (Join-Path $root "third_party\ffmpeg\bin\ffprobe.exe") -v error -show_entries format=duration -of csv=p=0 $file.FullName
        Write-Journal "fichier $($file.Name) : $([math]::Round($file.Length / 1MB)) Mo, $probe s"
        if (-not $probe -or [double]$probe -lt 10) { $failures += "$($file.Name) illisible ou trop court" }
    }
    $pending = Join-Path $config "recordings.txt"
    if (Test-Path $pending) { $failures += "journal non vidé : $(Get-Content $pending)" }
    Write-Journal "espace libre restant sur ${letter}: : $([math]::Round((Get-PSDrive $letter).Free / 1MB)) Mo"
    if ($failures.Count -eq 0) { Write-Journal "OK" } else { Write-Journal "ÉCHEC : $($failures -join ' ; ')" }
}
finally {
    foreach ($process in $started) {
        if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    }
    if ($attached) {
        Invoke-DiskPart @("select vdisk file=`"$vhdx`"", "detach vdisk")
    }
    Remove-Item $vhdx -Force -ErrorAction SilentlyContinue
    Remove-Item $config -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item Env:PALACLIPS_CONFIG_DIR -ErrorAction SilentlyContinue
    Write-Host "journal : $journal"
}
