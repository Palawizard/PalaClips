param(
    [int]$Before = 10,
    [int]$After = 10,
    [switch]$SkipSync,
    [switch]$SyncOnly,
    [string]$Name = "post-roll"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$isolated = Join-Path $root "target\test-$Name"
$clips = Join-Path $isolated "clips"
New-Item -ItemType Directory -Force $journalDir | Out-Null
Remove-Item $isolated -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $clips | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$started = @()

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Start-Replay([string]$Arguments) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $script:cli
    $info.Arguments = "replay $Arguments --clips-dir `"$clips`""
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
    $Replay.Process.StandardInput.WriteLine($Line)
    Start-Sleep -Milliseconds ([int]($WaitSeconds * 1000))
}

function Stop-Replay([object]$Replay, [string]$Label) {
    $Replay.Process.StandardInput.WriteLine("q")
    if (-not $Replay.Process.WaitForExit(60000)) { throw "$Label : le replay ne s'est pas arrêté" }
    Write-Journal "--- $Label"
    Write-Journal (($Replay.Stdout.Result -split "`n" | Select-String "clip|enregistrement|alerte|état") -join "`n")
    Write-Journal (($Replay.Stderr.Result -split "`n" | Select-String "WARN|ERROR") -join "`n")
}

function Test-Clips([string]$Label) {
    $failures = 0
    foreach ($file in Get-ChildItem $clips -Recurse -Filter *.mp4 | Sort-Object LastWriteTime) {
        $check = python -I -X utf8 (Join-Path $root "scripts\clip-continuity.py") $file.FullName --json | ConvertFrom-Json
        $starts = ffprobe -v error -show_entries stream=start_time -of csv=p=0 $file.FullName
        $durations = $check.streams | ForEach-Object { $_.duration_s }
        $spread = ($durations | Measure-Object -Maximum -Minimum | ForEach-Object { $_.Maximum - $_.Minimum })
        $ok = $check.continuous -and ($spread -le 0.05 -or $file.Name -notlike "*_replay*")
        if (-not $ok) { $failures++ }
        $detail = ($check.streams | ForEach-Object { "$($_.type) $($_.packets) paq. doublons $($_.duplicates) trous $($_.gaps) (max $($_.largest_gap_ms) ms à $($_.gaps_at_s -join "/") s)" }) -join " ; "
        Write-Journal ("{0} [{1}] : vidéo {2:N3} s, écart entre flux {3:N3} s, débuts {4} ; {5} → {6}" -f $file.Name, $Label, $durations[0], $spread, ($starts -join "/"), $detail, $(if ($ok) { "OK" } else { "ÉCHEC" }))
        Move-Item $file.FullName (Join-Path $isolated "$Label-$($file.Name)")
    }
    $failures
}

try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $common = "--target primary --height 720 --fps 60 --bitrate-mbps 8 --duration $Before --after $After --no-mic --no-game"
    Write-Journal "$(Get-Date -Format s) $Name : avant $Before s, après $After s, écran principal 720p60 8 Mb/s, piste mix ; dossier isolé $isolated"
    $failures = 0

    if (-not $SyncOnly) {
        $replay = Start-Replay $common
        Start-Sleep -Seconds ($Before + 4)
        Send $replay "" ($After + 4)
        $failures += Test-Clips "simple"
        Send $replay "" ([math]::Floor($After / 2))
        Send $replay "" ($After + 4)
        $failures += Test-Clips "prolonge"
        Send $replay "" 3
        Send $replay "f" 4
        $failures += Test-Clips "termine"
        Send $replay "" 2
        Send $replay "e" 4
        Send $replay "e" ($After + 4)
        $failures += Test-Clips "avec-enregistrement"
        Send $replay "" 2
        Send $replay "r" ($After + 6)
        $failures += Test-Clips "source-changee"
        Stop-Replay $replay "RAM"

        $replay = Start-Replay "$common --disk"
        Start-Sleep -Seconds ($Before + 4)
        Send $replay "" ($After + 4)
        $failures += Test-Clips "disque"
        Stop-Replay $replay "disque"

        $replay = Start-Replay ($common -replace "--after $After", "--after 60")
        Start-Sleep -Seconds ($Before + 4)
        Send $replay "" 6
        $replay.Process.Kill()
        $replay.Process.WaitForExit()
        Write-Journal "--- arrêt brutal 6 s après l'appui (après = 60 s) : processus tué"
        $replay = Start-Replay "--no-arm"
        Start-Sleep -Seconds 6
        Stop-Replay $replay "redémarrage (récupération)"
        $failures += Test-Clips "recupere"
    }

    if (-not $SkipSync) {
        foreach ($case in @(@{ Label = "reference"; After = 0 }, @{ Label = "synchro"; After = $After })) {
            $replay = Start-Replay ($common -replace "--after $After", "--after $($case.After)")
            Start-Sleep -Seconds 4
            $emitter = Start-Process $cli -ArgumentList "probe-av-emitter", "--seconds", "$($Before + $case.After + 6)" -PassThru -WindowStyle Hidden
            $started += $emitter
            Start-Sleep -Seconds ($Before + 3)
            Send $replay "" ($case.After + 6)
            $emitter.WaitForExit(30000) | Out-Null
            $file = Get-ChildItem $clips -Recurse -Filter *.mp4 | Select-Object -First 1
            if ($file) {
                $copy = Join-Path $isolated "$($case.Label).mp4"
                Copy-Item $file.FullName $copy
                $length = [double](ffprobe -v error -select_streams v:0 -show_entries stream=duration -of csv=p=0 $copy)
                $title = if ($case.After -gt 0) { "jonction vers $([math]::Round($length - $case.After, 1)) s" } else { "clip instantané de référence" }
                Write-Journal "synchro $($case.Label) : $title"
                Write-Journal ((python -I -X utf8 (Join-Path $root "scripts\measure-av-sync.py") $copy --audio 0 --window 5) -join "`n")
            }
            $failures += Test-Clips $case.Label
            Stop-Replay $replay $case.Label
        }
    }
    Write-Journal "RÉSULTAT : $failures clip(s) hors critères (continuité des PTS, pistes de même durée)"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $isolated -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : replays et émetteur arrêtés, dossier isolé supprimé, journal $journal"
}
