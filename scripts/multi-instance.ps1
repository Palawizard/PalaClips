param(
    [string]$Device = "Realtek",
    [string]$Name = "multi-instance"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$work = Join-Path $root "target\$Name"
$clips = Join-Path $work "clips"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Start-Tone([double]$Hz) {
    Start-Process (Join-Path $work "tone.exe") -ArgumentList @("play-tone", "--hz", $Hz, "--amplitude", "0.05", "--seconds", "25", "--device", $Device) -WindowStyle Hidden -PassThru
}

$started = @()
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $clips | Out-Null
    Copy-Item (Join-Path $root "target\release\palaclips-cli.exe") (Join-Path $work "tone.exe")
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = Join-Path $root "target\release\palaclips-cli.exe"
    $info.Arguments = "replay --height 360 --fps 30 --bitrate-mbps 1 --duration 30 --no-mic --no-mix --no-game --app tone.exe --clips-dir `"$clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $info.StandardErrorEncoding = [Text.Encoding]::UTF8
    $replay = [System.Diagnostics.Process]::Start($info)
    $started += $replay
    $stdout = $replay.StandardOutput.ReadToEndAsync()
    $stderr = $replay.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 4
    if ($replay.HasExited) { throw "replay non armé : $($stderr.Result.Trim())" }

    Write-Journal "$(Get-Date -Format s) $Name : deux instances indépendantes de tone.exe (440 Hz puis 660 Hz, sortie $Device), piste d'appli tone.exe"
    $started += Start-Tone 440
    Start-Sleep -Seconds 4
    $started += Start-Tone 660
    Start-Sleep -Seconds 8
    $replay.StandardInput.WriteLine("s")
    Start-Sleep -Seconds 4
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(30000)) { throw "le replay ne s'est pas arrêté" }

    $lines = ($stdout.Result + "`n" + $stderr.Result) -split "`n" | ForEach-Object { $_ -replace "`e\[[0-9;]*m", '' }
    $lines | Select-String "rattachée|instance|clip sauvegardé|muette|piste audio arrêtée" | ForEach-Object { Write-Journal $_.Line.Trim() }
    $clip = Get-ChildItem $clips -Recurse -Filter *.mp4 | Select-Object -First 1
    if (-not $clip) { throw "aucun clip sauvegardé" }
    $levels = python (Join-Path $PSScriptRoot "measure-tones.py") $clip.FullName --hz 440 --hz 660 --json | ConvertFrom-Json
    $track = $levels | Select-Object -First 1
    $second = 0
    Write-Journal "piste « $($track.title) » (dBFS) : $(($track.seconds | ForEach-Object { "$(($second++))s 440=$($_.'440.0') 660=$($_.'660.0')" }) -join ' | ')"
    $last = $track.seconds | Select-Object -Last 4
    $both = @($last | Where-Object { $null -ne $_.'440.0' -and $null -ne $_.'660.0' -and $_.'440.0' -gt -60 -and $_.'660.0' -gt -60 }).Count
    Write-Journal "RÉSULTAT : $(if ($both -ge 3) { "les deux instances sont dans la piste ($both secondes sur 4 à la fin)" } else { "ÉCHEC, une instance manque ($both secondes sur 4)" })"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Get-Process tone -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$work*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : sons, replay et fichiers de test supprimés, journal $journal"
}
