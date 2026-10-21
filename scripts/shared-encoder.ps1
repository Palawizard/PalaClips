param([string]$Name = "shared-encoder")

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$clips = Join-Path $root "target\$Name"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Get-Sessions {
    $samples = 1..5 | ForEach-Object {
        [int](nvidia-smi --query-gpu=encoder.stats.sessionCount --format=csv,noheader).Trim()
        Start-Sleep -Milliseconds 400
    }
    ($samples | Measure-Object -Maximum).Maximum
}

$replay = $null
try {
    if (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in @("palaclips-app", "PalaClips") }) {
        throw "PalaClips tourne : son encodeur fausserait le compte"
    }
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $clips | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $idle = Get-Sessions
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = Join-Path $root "target\release\palaclips-cli.exe"
    $info.Arguments = "replay --height 360 --fps 30 --bitrate-mbps 1 --duration 30 --no-mic --no-game --clips-dir `"$clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $replay = [System.Diagnostics.Process]::Start($info)
    $stdout = $replay.StandardOutput.ReadToEndAsync()
    $stderr = $replay.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 5
    if ($replay.HasExited) { throw "replay non armé : $($stderr.Result.Trim())" }
    $armed = Get-Sessions
    $replay.StandardInput.WriteLine("e")
    Start-Sleep -Seconds 5
    $recording = Get-Sessions
    $replay.StandardInput.WriteLine("e")
    Start-Sleep -Seconds 3
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(30000)) { throw "le replay ne s'est pas arrêté" }
    $files = @(Get-ChildItem $clips -Recurse -Filter *.mp4)
    Write-Journal "$(Get-Date -Format s) $Name : sessions NVENC (nvidia-smi, maximum sur 5 relevés) au repos $idle, replay armé $armed, replay + enregistrement $recording ; fichiers écrits : $($files.Count)"
    Write-Journal "RÉSULTAT : $(if ($armed -eq $idle + 1 -and $recording -eq $armed -and $files.Count -ge 1) { 'un seul encodeur pour le replay et l''enregistrement' } else { 'ÉCHEC : l''enregistrement ouvre un encodeur de plus ou rien n''est écrit' })"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    if ($replay -and -not $replay.HasExited) { Stop-Process -Id $replay.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : replay arrêté, fichiers supprimés, journal $journal"
}
