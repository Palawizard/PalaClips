param(
    [int]$CrashAt = 8,
    [int]$RestartAt = 16,
    [int]$SaveAt = 28,
    [string]$ToneDevice = "Realtek",
    [string]$Name = "app-crash"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$clips = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir | Out-Null
Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Start-Tone {
    Start-Process $script:tone -ArgumentList "play-tone", "--hz", "440", "--seconds", "120", "--device", $ToneDevice -PassThru -NoNewWindow
}

$started = @()
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $script:tone = Join-Path $root "target\tone-crash.exe"
    Copy-Item $cli $script:tone -Force
    Write-Journal "$(Get-Date -Format s) $Name : piste d'appli tone-crash.exe (440 Hz sur « $ToneDevice »), processus tué à $CrashAt s, relancé à $RestartAt s, sauvegarde à $SaveAt s (replay 30 s, écran principal 360p30)"
    $started += Start-Tone
    Start-Sleep -Seconds 2

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = "replay --target primary --height 360 --fps 30 --bitrate-mbps 1 --duration 30 --no-mic --no-mix --no-game --app tone-crash.exe --clips-dir `"$clips`""
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
    $clock = [Diagnostics.Stopwatch]::StartNew()

    while ($clock.Elapsed.TotalSeconds -lt $CrashAt) { Start-Sleep -Milliseconds 100 }
    Stop-Process -Id $started[0].Id -Force
    Write-Journal "$([math]::Round($clock.Elapsed.TotalSeconds, 1)) s : tone-crash.exe tué (Stop-Process -Force)"
    while ($clock.Elapsed.TotalSeconds -lt $RestartAt) { Start-Sleep -Milliseconds 100 }
    $started += Start-Tone
    Write-Journal "$([math]::Round($clock.Elapsed.TotalSeconds, 1)) s : tone-crash.exe relancé (PID $($started[-1].Id))"
    while ($clock.Elapsed.TotalSeconds -lt $SaveAt) { Start-Sleep -Milliseconds 100 }
    $replay.StandardInput.WriteLine("")
    Start-Sleep -Seconds 3
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(60000)) { throw "le replay ne s'est pas arrêté" }
    Write-Journal "replay terminé, code $($replay.ExitCode)"
    Write-Journal $stdout.Result.TrimEnd()
    Write-Journal ($stderr.Result -split "`n" | Select-String "WARN|ERROR|rattach|panique" | Out-String).TrimEnd()

    $clip = Get-ChildItem $clips -Recurse -Filter *.mp4 | Select-Object -First 1
    if (-not $clip) { throw "aucun clip sauvegardé" }
    $levels = python -I -X utf8 (Join-Path $root "scripts\measure-tones.py") $clip.FullName --hz 440 --json | ConvertFrom-Json
    $present = $levels[0].seconds | ForEach-Object { $null -ne $_."440.0" -and $_."440.0" -gt -40 }
    $runs = @()
    foreach ($second in $present) {
        if ($runs.Count -gt 0 -and $runs[-1].present -eq $second) { $runs[-1].length++ }
        else { $runs += [pscustomobject]@{ present = $second; length = 1 } }
    }
    $pattern = ($runs | ForEach-Object { "$(if ($_.present) { 'son' } else { 'silence' }) $($_.length) s" }) -join " → "
    Write-Journal "piste « $($levels[0].title) » du clip ($($present.Count) s) : $pattern"
    $absent = $runs | Where-Object { -not $_.present }
    $gap = $RestartAt - $CrashAt
    $ok = ($replay.ExitCode -eq 0) -and ($runs.Count -eq 3) -and $runs[0].present -and $runs[2].present -and ($absent.Count -eq 1) -and ([math]::Abs($absent[0].length - $gap) -le 2)
    Write-Journal "RÉSULTAT : $(if ($ok) { 'OK' } else { 'ÉCHEC' }) (attendu : son, silence d'environ $gap s, son ; replay jamais arrêté)"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : sons de test et replay arrêtés, clip supprimé, journal $journal"
}
