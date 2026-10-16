param(
    [int]$Minutes = 60,
    [string]$OutDir = "target\clips",
    [switch]$NoPause,
    [switch]$CalibrateOnly
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
cargo build --release -p palaclips-cli
if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
$env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
$cli = Join-Path $root "target\release\palaclips-cli.exe"
New-Item -ItemType Directory -Force $OutDir | Out-Null

function Invoke-SyncCapture {
    param([int]$Seconds, [string]$File, [string[]]$Extra, [int[]]$EmitAt, [int]$SilenceAt = -1)
    $job = Start-Job -ScriptBlock {
        param($root, $cli, $seconds, $file, $extra)
        Set-Location $root
        $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
        & $cli capture-av --target primary --seconds $seconds --out $file @extra 2>&1 | Out-String -Width 300
    } -ArgumentList $root, $cli, $Seconds, $File, $Extra
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $events = @($EmitAt | ForEach-Object { [pscustomobject]@{ At = $_; Kind = "emit" } })
    if ($SilenceAt -ge 0) { $events += [pscustomobject]@{ At = $SilenceAt; Kind = "silence" } }
    foreach ($event in $events | Sort-Object At) {
        $wait = $event.At - $clock.Elapsed.TotalSeconds
        if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]($wait * 1000)) }
        if ($event.Kind -eq "silence") {
            [console]::Beep(880, 400)
            Write-Host "$(Get-Date -Format T)  coupe tout son maintenant, pendant 2 min (jusqu'au prochain flash)"
        } else {
            Write-Host "$(Get-Date -Format T)  émetteur flash + bip pendant 20 s"
            & $cli probe-av-emitter --seconds 20 | Out-Null
        }
    }
    Receive-Job $job -Wait -AutoRemoveJob
}

function Measure-Track([string]$File, [int]$Track) {
    python -I -X utf8 (Join-Path $root "scripts\measure-av-sync.py") $File --audio $Track --window 60 --json | ConvertFrom-Json
}

$calibration = Join-Path $OutDir "t2-calibrage.mp4"
Write-Host "Calibrage (35 s) : casque ou enceintes près du micro, écran principal libre."
Invoke-SyncCapture -Seconds 35 -File $calibration -Extra @() -EmitAt @(8)
$mix = Measure-Track $calibration 0
$mic = Measure-Track $calibration 1
if ($null -eq $mix.mean_ms) { throw "aucun bip détecté sur la piste mix : vérifie que le son sort bien de la sortie par défaut" }
$mixOffset = [int][math]::Round(-$mix.mean_ms)
$micOffset = 0
if ($null -eq $mic.mean_ms) {
    Write-Warning "le micro n'a entendu aucun bip : rapproche le casque ou les enceintes du micro et relance, sinon la piste micro ne sera pas mesurée"
} else {
    $micOffset = [int][math]::Round(-$mic.mean_ms)
}
Write-Host "Décalages mesurés : mix $($mix.mean_ms) ms, micro $($mic.mean_ms) ms -> options --mix-offset-ms $mixOffset --mic-offset-ms $micOffset"
if ($CalibrateOnly) { return }
if (-not $NoPause) { Read-Host "Entrée pour lancer la capture de $Minutes min" }

$file = Join-Path $OutDir "t2.mp4"
$half = $Minutes * 30
Invoke-SyncCapture -Seconds ($Minutes * 60 + 40) -File $file `
    -Extra @("--mix-offset-ms", "$mixOffset", "--mic-offset-ms", "$micOffset") `
    -EmitAt @(10, $half, ($Minutes * 60)) -SilenceAt ($half - 120)

ffprobe -v error -show_entries stream=index,codec_name,start_time,duration,nb_frames:stream_tags=handler_name -of compact $file
foreach ($track in 0, 1) {
    $result = Measure-Track $file $track
    $name = @("mix", "micro")[$track]
    if ($result.windows.Count -eq 0) { Write-Host "$name : aucun bip détecté, piste non mesurée" }
    foreach ($window in $result.windows) {
        $verdict = if ([math]::Abs($window.mean_ms) -lt 16.7) { "OK" } else { "HORS TOLÉRANCE" }
        Write-Host ("{0,-5} {1,6} s : {2,3} paires, min {3,7:+0.0;-0.0} ms, moy {4,7:+0.0;-0.0} ms, max {5,7:+0.0;-0.0} ms  {6}" -f $name, $window.start_s, $window.pairs, $window.min_ms, $window.mean_ms, $window.max_ms, $verdict)
    }
}
