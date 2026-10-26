param(
    [ValidateSet("output", "desktop", "tracks", "all")]
    [string]$Mode = "all",
    [string]$Extra = "",
    [double]$Threshold = 0.6,
    [switch]$Keep,
    [string]$Name = "feedback-sounds"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$isolated = Join-Path $root "target\test-$Name"
New-Item -ItemType Directory -Force $journalDir | Out-Null
Remove-Item $isolated -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $isolated | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$started = @()
$found = 0
$witnesses = 0
$summary = @()

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Start-Replay([string]$Arguments, [string]$Clips) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $script:cli
    $info.Arguments = "replay $Arguments --clips-dir `"$Clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $info.EnvironmentVariables["PALACLIPS_CONFIG_DIR"] = $isolated
    $process = [System.Diagnostics.Process]::Start($info)
    $script:started += $process
    [pscustomobject]@{
        Process = $process
        Stdout = $process.StandardOutput.ReadToEndAsync()
        Stderr = $process.StandardError.ReadToEndAsync()
    }
}

function Send([object]$Replay, [string]$Line, [double]$WaitSeconds) {
    Write-Journal ("  {0:N1} s : « {1} »" -f $script:clock.Elapsed.TotalSeconds, $Line)
    $Replay.Process.StandardInput.WriteLine($Line)
    Start-Sleep -Milliseconds ([int]($WaitSeconds * 1000))
}

function Measure-Clips([string]$Clips, [string]$Label) {
    foreach ($file in Get-ChildItem $Clips -Recurse -Filter *.mp4 | Sort-Object Name) {
        $tracks = python -I -X utf8 (Join-Path $root "scripts\find-chimes.py") $file.FullName --threshold $Threshold --json | ConvertFrom-Json
        Write-Journal ("{0} [{1}] : {2:N1} s" -f $file.Name, $Label, $tracks[0].duration_s)
        foreach ($track in $tracks) {
            $own = @($track.chimes | Where-Object { $_.sound -ne "fin d'enregistrement" })
            $witness = @($track.chimes | Where-Object { $_.sound -eq "fin d'enregistrement" })
            $script:found += $own.Count
            if ($track.index -eq 0) { $script:witnesses += $witness.Count }
            $verdict = if ($own) {
                "SON DE PALACLIPS : " + (($own | ForEach-Object { "{0} à {1:N2} s (corrélation {2:N2})" -f $_.sound, $_.at_s, $_.score }) -join ", ")
            } else { "aucun son de PalaClips" }
            if ($witness) { $verdict += " ; témoin d'un autre processus à " + (($witness | ForEach-Object { "{0:N2} s ({1:N2})" -f $_.at_s, $_.score }) -join ", ") }
            Write-Journal ("  piste {0} « {1} » : {2}" -f ($track.index + 1), $track.title, $verdict)
        }
    }
}

function Invoke-Mode([string]$ModeName) {
    $clips = Join-Path $isolated "clips-$ModeName"
    New-Item -ItemType Directory -Force $clips | Out-Null
    $mix = switch ($ModeName) { "tracks" { "--tracks-mix" } "output" { "--mix-output" } default { "" } }
    $arguments = "--target primary --height 720 --fps 30 --bitrate-mbps 4 --duration 30 --after 5 --no-game --chime $mix $Extra"
    Write-Journal "--- mode $ModeName : replay $arguments"
    $script:clock = [Diagnostics.Stopwatch]::StartNew()
    $replay = Start-Replay $arguments $clips
    Start-Sleep -Seconds 8
    Send $replay "" 4
    Start-Process -FilePath $script:cli -ArgumentList "play-chime --stopped" -NoNewWindow -RedirectStandardOutput (Join-Path $isolated "external.txt")
    Write-Journal ("  {0:N1} s : son de fin d'enregistrement joué par un autre processus (témoin)" -f $script:clock.Elapsed.TotalSeconds)
    Start-Sleep -Seconds 8
    Send $replay "" 12
    Send $replay "e" 8
    Send $replay "e" 4
    $replay.Process.StandardInput.WriteLine("q")
    if (-not $replay.Process.WaitForExit(60000)) { throw "mode $ModeName : le replay ne s'est pas arrêté" }
    Write-Journal (($replay.Stdout.Result -split "`n" | Select-String "capture :|clip|état|alerte") -join "`n")
    Write-Journal (($replay.Stderr.Result -split "`n" | Select-String "WARN|ERROR") -join "`n")
    $script:found = 0
    $script:witnesses = 0
    Measure-Clips $clips $ModeName
    $script:summary += "mode ${ModeName} : $($script:found) son(s) de PalaClips, témoin entendu $($script:witnesses) fois dans la piste 1"
}

try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    Write-Journal "$(Get-Date -Format s) $Name : sons de PalaClips joués par la CLI elle-même (comme l'app), corrélation minimale $Threshold ; dossier isolé $isolated"
    $modes = if ($Mode -eq "all") { @("output", "desktop", "tracks") } else { @($Mode) }
    foreach ($modeName in $modes) { Invoke-Mode $modeName }
    $summary | ForEach-Object { Write-Journal "RÉSUMÉ $_" }
}
finally {
    foreach ($process in $started) {
        if (-not $process.HasExited) { $process.Kill($true) }
    }
    if (-not $Keep) {
        Get-ChildItem $isolated -Recurse -Filter *.mp4 -ErrorAction SilentlyContinue | Remove-Item -Force
    }
    Write-Journal "journal : $journal"
}
