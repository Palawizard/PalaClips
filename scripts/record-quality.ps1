param(
    [string[]]$ReplayArgs = @("--height", "480", "--fps", "60", "--bitrate-mbps", "4"),
    [string[]]$RecordArgs = @("--record-height", "720", "--record-fps", "30", "--record-codec", "hevc", "--record-bitrate-mbps", "6"),
    [string[]]$Extra = @(),
    [int]$Seconds = 12,
    [string]$Name = "record-quality",
    [switch]$Keep,
    [switch]$AvSync
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$cli = Join-Path $root "target\release\palaclips-cli.exe"
$clips = Join-Path $root "target\$Name"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

$process = $null
try {
    if (-not (Test-Path $cli)) { throw "compile d'abord : cargo build --release -p palaclips-cli" }
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $clips | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $arguments = @("replay", "--target", "primary", "--duration", "30", "--no-mic", "--no-game", "--clips-dir", "`"$clips`"") + $ReplayArgs + $RecordArgs + $Extra
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = $arguments -join " "
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($info)
    $output = $process.StandardOutput.ReadToEndAsync()
    $errors = $process.StandardError.ReadToEndAsync()
    Write-Journal "$(Get-Date -Format s) $Name : replay [$($ReplayArgs -join ' ')], enregistrement [$($RecordArgs -join ' ')] $($Extra -join ' ')"
    Start-Sleep -Seconds 8
    $process.StandardInput.WriteLine("e")
    if ($AvSync) {
        Start-Sleep -Seconds 2
        & $cli probe-av-emitter --seconds 12 | Out-Null
        Start-Sleep -Seconds 1
        $process.StandardInput.WriteLine("")
        Start-Sleep -Seconds 2
    } else {
        Start-Sleep -Seconds ([math]::Floor($Seconds / 2))
        $process.StandardInput.WriteLine("")
        Start-Sleep -Seconds ([math]::Ceiling($Seconds / 2))
    }
    $process.StandardInput.WriteLine("e")
    Start-Sleep -Seconds 4
    $process.StandardInput.WriteLine("q")
    if (-not $process.WaitForExit(60000)) { throw "la CLI ne s'arrête pas" }
    $log = ($output.Result + $errors.Result) -replace "`e\[[0-9;]*m", ''
    $log -split "`n" | Where-Object { $_ -match "2e encodage|propre qualité|refusé|ERROR|WARN palaclips" } | ForEach-Object { Write-Journal "  $($_.Trim())" }
    $files = @(Get-ChildItem $clips -Recurse -Filter *.mp4 | Sort-Object Name)
    if ($files.Count -lt 2) { throw "attendu un enregistrement et au moins un replay, trouvé $($files.Count) fichier(s)" }
    foreach ($file in $files) {
        $video = ffprobe -v error -select_streams v -show_entries stream=codec_name,width,height,r_frame_rate,avg_frame_rate -of csv=p=0 $file.FullName
        $duration = ffprobe -v error -show_entries format=duration -of csv=p=0 $file.FullName
        $continuity = (python (Join-Path $PSScriptRoot "clip-continuity.py") $file.FullName) -join " ; "
        Write-Journal ("{0} : {1}, {2:N2} s | {3}" -f $file.Name, $video, [double]$duration, $continuity)
        if ($AvSync) {
            $sync = python -I -X utf8 (Join-Path $PSScriptRoot "measure-av-sync.py") $file.FullName --audio 0 --json | ConvertFrom-Json
            Write-Journal ("  synchro A/V (piste 1) : {0} paires, décalage moyen {1} ms, dispersion {2} ms" -f $sync.pairs, $sync.mean_ms, $sync.fit.dispersion_ms)
        }
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    if ($process -and -not $process.HasExited) { $process.Kill() }
    if (-not $Keep) { Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue }
    Write-Journal "$(Get-Date -Format s) fin : CLI arrêtée, clips supprimés, journal $journal"
}
