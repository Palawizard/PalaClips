param(
    [ValidateSet("desktop", "noise")][string]$Content = "desktop",
    [int]$Seconds = 30,
    [string]$Device = "Realtek",
    [string]$Name = "size-estimate"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$cli = Join-Path $root "target\release\palaclips-cli.exe"
$work = Join-Path $root "target\$Name"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$Content-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

$started = @()
try {
    if (-not (Test-Path $cli)) { throw "compile d'abord : cargo build --release -p palaclips-cli" }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $work | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    foreach ($index in 1..4) {
        $exe = Join-Path $work ("tone-{0}.exe" -f $index)
        Copy-Item $cli $exe
        $started += Start-Process $exe -ArgumentList "play-tone", "--hz", "$(300 + 90 * $index)", "--amplitude", "0.1", "--seconds", "600", "--device", $Device -PassThru -WindowStyle Hidden
    }
    if ($Content -eq "noise") {
        $started += Start-Process (Join-Path $root "third_party\ffmpeg\bin\ffplay.exe") -ArgumentList "-hide_banner", "-loglevel", "error", "-an", "-fs", "-f", "lavfi", "-i", "nullsrc=s=640x360:r=60,geq=random(1)*255:128:128,scale=2560:1440:flags=neighbor" -PassThru
        Start-Sleep -Seconds 3
    }
    Write-Journal "$(Get-Date -Format s) $Name : contenu « $Content », qualité Équilibré, $Seconds s par essai, écran principal"
    $runs = @(
        @{ Height = 1080; Fps = 30 }, @{ Height = 1080; Fps = 60 }, @{ Height = 1440; Fps = 60 }
    )
    foreach ($run in $runs) {
        foreach ($tracks in 2, 6) {
            $out = Join-Path $work "capture.mp4"
            $arguments = @("capture-apps", "--target", "primary", "--height", "$($run.Height)", "--fps", "$($run.Fps)", "--quality", "equilibre", "--mic", "--seconds", "$Seconds", "--out", $out)
            if ($tracks -eq 6) { foreach ($index in 1..4) { $arguments += @("--app", "tone-$index.exe") } }
            $log = & $cli @arguments 2>&1 | Out-String
            if (-not (Test-Path $out)) { throw "pas de fichier : $log" }
            $width = [int](ffprobe -v error -select_streams v -show_entries stream=width -of csv=p=0 $out)
            $height = [int](ffprobe -v error -select_streams v -show_entries stream=height -of csv=p=0 $out)
            $streams = @(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 $out).Count
            $duration = [double](ffprobe -v error -show_entries format=duration -of csv=p=0 $out)
            $kbps = [math]::Min(200000, [math]::Max(1000, [math]::Round($width * $height * $run.Fps * 0.18 / 1000)))
            $estimate = [double]((& $cli estimate --video-kbps $kbps --audio-tracks $streams | Select-String "octets par minute").Line -replace '\D', '')
            $actual = (Get-Item $out).Length / $duration * 60
            Write-Journal ("{0}x{1} {2} i/s, {3} pistes ({4} kb/s vidéo) : estimé {5:N1} Mo/min, mesuré {6:N1} Mo/min, écart {7:+0.0;-0.0} %" -f $width, $height, $run.Fps, $streams, $kbps, ($estimate / 1e6), ($actual / 1e6), (100 * ($actual - $estimate) / $estimate))
            Remove-Item $out -Force
        }
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Get-Process palaclips-cli -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : sons et fenêtres arrêtés, fichiers supprimés, journal $journal"
}
