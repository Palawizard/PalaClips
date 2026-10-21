param(
    [string]$Target = "window:FortniteClient-Win64-Shipping.exe",
    [int]$Seconds = 30,
    [int]$Fps = 60,
    [int]$ReferenceMbps = 150,
    [int]$BitrateMbps = 40,
    [string]$Name = "nvenc-quality"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$work = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir, $work | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$ffmpeg = Join-Path $root "third_party\ffmpeg\bin\ffmpeg.exe"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Measure-Quality([string]$Distorted, [string]$Reference) {
    $graph = "[0:v]setpts=PTS-STARTPTS,split=3[d1][d2][d3];[1:v]setpts=PTS-STARTPTS,split=3[r1][r2][r3];[d1][r1]libvmaf=n_threads=8;[d2][r2]psnr;[d3][r3]ssim"
    $output = & $ffmpeg -hide_banner -i $Distorted -i $Reference -lavfi $graph -f null - 2>&1 | Out-String
    $vmaf = if ($output -match 'VMAF score: ([\d.]+)') { [double]$Matches[1] } else { [double]::NaN }
    $psnr = [double]::NaN; $psnrMin = [double]::NaN
    if ($output -match 'PSNR y:[\d.]+ u:[\d.]+ v:[\d.]+ average:([\d.]+) min:([\d.]+)') {
        $psnr = [double]$Matches[1]; $psnrMin = [double]$Matches[2]
    }
    $ssim = if ($output -match 'SSIM Y:[\d.]+ .* All:([\d.]+)') { [double]$Matches[1] } else { [double]::NaN }
    [pscustomobject]@{ Vmaf = $vmaf; Psnr = $psnr; PsnrMin = $psnrMin; Ssim = $ssim }
}

try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $reference = Join-Path $work "reference.mp4"
    Write-Journal "$(Get-Date -Format s) $Name : $Target, $Seconds s à $Fps i/s, référence $ReferenceMbps Mb/s (réglages PalaClips), puis réencodage à $BitrateMbps Mb/s : défauts FFmpeg contre ADR-022"
    $log = & $cli capture-video --target $Target --fps $Fps --bitrate-mbps $ReferenceMbps --seconds $Seconds --out $reference 2>&1 | Out-String
    if (-not (Test-Path $reference)) { throw "capture impossible : $($log.Trim())" }
    $log -split "`r?`n" | Where-Object { $_ -match 'i/s|arrêtée|WARN|ERROR' } | ForEach-Object { Write-Journal ($_ -replace '^\S+Z\s+', '') }

    $common = @("-c:v", "h264_nvenc", "-preset", "p4", "-rc", "cbr", "-b:v", "$($BitrateMbps)M", "-maxrate", "$($BitrateMbps)M", "-bufsize", "$($BitrateMbps)M", "-g", "$Fps", "-bf", "0")
    $variants = [ordered]@{
        "défauts FFmpeg" = @()
        "ADR-022 (dpb_size=2, surfaces=2, delay=1)" = @("-dpb_size", "2", "-surfaces", "2", "-delay", "1")
    }
    $results = @()
    $index = 0
    foreach ($label in $variants.Keys) {
        $index++
        $encoded = Join-Path $work "variant$index.mp4"
        & $ffmpeg -hide_banner -loglevel error -y -i $reference -an @common @($variants[$label]) $encoded
        if ($LASTEXITCODE -ne 0) { throw "réencodage « $label » impossible" }
        $quality = Measure-Quality $encoded $reference
        $size = (Get-Item $encoded).Length / 1e6
        Write-Journal ("{0,-44} VMAF {1,6:N2} | PSNR moy {2,6:N2} dB, min {3,6:N2} dB | SSIM {4:N5} | {5,6:N1} Mo" -f $label, $quality.Vmaf, $quality.Psnr, $quality.PsnrMin, $quality.Ssim, $size)
        $results += $quality
    }
    Write-Journal ("ÉCART ADR-022 − défauts : VMAF {0:+0.00;-0.00}, PSNR {1:+0.00;-0.00} dB, SSIM {2:+0.00000;-0.00000}" -f ($results[1].Vmaf - $results[0].Vmaf), ($results[1].Psnr - $results[0].Psnr), ($results[1].Ssim - $results[0].Ssim))
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : vidéos supprimées, journal $journal"
}
