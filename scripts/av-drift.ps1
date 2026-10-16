param(
    [int]$Minutes = 20,
    [string]$Device = "Apogee",
    [int]$Fps = 60,
    [int]$Height = 360,
    [int]$BitrateMbps = 2,
    [string]$Name = "av-drift"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$outDir = Join-Path $root "target\clips"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $outDir, $journalDir | Out-Null
$stamp = Get-Date -Format "yyyy-MM-dd_HH-mm"
$journal = Join-Path $journalDir "$Name-$stamp.txt"
$video = Join-Path $outDir "$Name-$stamp.mp4"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

Add-Type -Namespace PalaClips -Name Power -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);'
$continuous = [uint32]2147483648
$keepAwake = $continuous -bor 1 -bor 2
[PalaClips.Power]::SetThreadExecutionState($keepAwake) | Out-Null
try {
    Write-Journal "$(Get-Date -Format s) $Name : $Minutes min, sortie « $Device » (émetteur et loopback), piste mix seule, $Height p $Fps i/s $BitrateMbps Mb/s"
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $seconds = $Minutes * 60
    $capture = Start-Job -ScriptBlock {
        param($root, $cli, $arguments)
        Set-Location $root
        $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
        & $cli @arguments 2>&1 | Out-String -Width 300
    } -ArgumentList $root, $cli, @(
        "capture-av", "--target", "primary", "--seconds", "$($seconds + 20)", "--out", $video,
        "--no-mic", "--mix-device", $Device, "--fps", "$Fps", "--height", "$Height", "--bitrate-mbps", "$BitrateMbps"
    )
    Start-Sleep -Seconds 5
    & $cli probe-av-emitter --seconds $seconds --device $Device 2>&1 | Out-String -Width 300 | ForEach-Object { Write-Journal $_.TrimEnd() }
    if ($LASTEXITCODE -ne 0) {
        Stop-Job $capture
        Write-Journal (Receive-Job $capture -AutoRemoveJob -Wait | Out-String).TrimEnd()
        throw "l'émetteur n'a pas pu jouer sur « $Device »"
    }
    Write-Journal (Receive-Job $capture -Wait -AutoRemoveJob).TrimEnd()
    if (-not (Test-Path $video)) { throw "aucune vidéo produite" }
    Write-Journal (ffprobe -v error -show_entries stream=index,codec_name,duration:stream_tags=handler_name -of compact $video | Out-String).TrimEnd()
    $result = python -I -X utf8 (Join-Path $root "scripts\measure-av-sync.py") $video --audio 0 --window 300 --json | ConvertFrom-Json
    foreach ($window in $result.windows) {
        Write-Journal ("{0,6} s : {1,3} paires, min {2,7:+0.0;-0.0} ms, moy {3,7:+0.0;-0.0} ms, max {4,7:+0.0;-0.0} ms" -f $window.start_s, $window.pairs, $window.min_ms, $window.mean_ms, $window.max_ms)
    }
    if ($result.fit) {
        Write-Journal ("RÉSULTAT {0} paires : pente {1:+0.00;-0.00} ± {2:0.00} ms/h, décalage à 0 s {3:+0.0;-0.0} ms, dispersion {4:0.0} ms" -f $result.pairs, $result.fit.slope_ms_per_h, $result.fit.slope_stderr_ms_per_h, $result.fit.offset_at_0_ms, $result.fit.dispersion_ms)
    } else {
        Write-Journal "RÉSULTAT : pas assez de paires flash/bip ($($result.flashes) flashs, $($result.beeps) bips)"
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    Remove-Item $video -ErrorAction SilentlyContinue
    [PalaClips.Power]::SetThreadExecutionState($continuous) | Out-Null
    Write-Journal "$(Get-Date -Format s) fin, vidéo supprimée, journal $journal"
}
