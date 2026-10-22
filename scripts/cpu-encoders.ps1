param(
    [string]$Source = "F:\Fast Documents\Vidéos\NVIDIA\Rocket League\Rocket League 2026.09.15 - 17.01.55.01.mp4",
    [double]$StartSeconds = 300,
    [int]$Seconds = 10,
    [string[]]$Formats = @("720p30", "1080p30", "1080p60"),
    [string]$Name = "cpu-encoders"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$work = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir, $work | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$ffmpeg = Join-Path $root "third_party\ffmpeg\bin\ffmpeg.exe"
$logical = [Environment]::ProcessorCount

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Get-Encoders([int]$Fps, [int]$Kbps) {
    $rate = @("-b:v", "$($Kbps)k", "-g", "$Fps")
    [ordered]@{
        "h264_nvenc p4 (référence GPU)" = @("-c:v", "h264_nvenc", "-preset", "p4", "-rc", "cbr", "-bf", "0") + $rate
        "libopenh264 (4 tranches, 4 fils)" = @("-c:v", "libopenh264", "-rc_mode", "bitrate", "-slices", "4", "-threads", "4") + $rate
        "h264_mf logiciel (Windows)" = @("-c:v", "h264_mf", "-hw_encoding", "0", "-rate_control", "cbr", "-pix_fmt", "nv12") + $rate
        "libsvtav1 M8 rtc" = @("-c:v", "libsvtav1", "-preset", "8", "-svtav1-params", "rtc=1:rc=2") + $rate
        "libsvtav1 M10 rtc" = @("-c:v", "libsvtav1", "-preset", "10", "-svtav1-params", "rtc=1:rc=2") + $rate
        "libsvtav1 M12 rtc" = @("-c:v", "libsvtav1", "-preset", "12", "-svtav1-params", "rtc=1:rc=2") + $rate
        "libvpx-vp9 realtime 8 (6 fils)" = @("-c:v", "libvpx-vp9", "-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1", "-threads", "6") + $rate
        "libaom-av1 realtime 8 (6 fils)" = @("-c:v", "libaom-av1", "-usage", "realtime", "-cpu-used", "8", "-row-mt", "1", "-threads", "6") + $rate
    }
}

function Invoke-Timed([string]$Reference, [string[]]$Arguments, [string]$Output) {
    $errors = Join-Path $work "stderr.txt"
    $all = @("-hide_banner", "-loglevel", "error", "-y", "-i", $Reference) + $Arguments + @($Output)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $ffmpeg -ArgumentList $all -NoNewWindow -PassThru -RedirectStandardError $errors
    $null = $process.Handle
    $process.WaitForExit()
    $clock.Stop()
    [pscustomobject]@{
        Wall = $clock.Elapsed.TotalSeconds
        Cpu = $process.TotalProcessorTime.TotalSeconds
        ExitCode = $process.ExitCode
        Errors = (Get-Content $errors -Raw -ErrorAction SilentlyContinue)
    }
}

function Measure-Quality([string]$Distorted, [string]$Reference, [int]$Fps) {
    $align = "setpts=N/($Fps*TB)"
    $graph = "[0:v]$align,split=3[d1][d2][d3];[1:v]$align,split=3[r1][r2][r3];[d1][r1]libvmaf=n_threads=16:shortest=1:model=version=vmaf_v0.6.1neg;[d2][r2]psnr=shortest=1;[d3][r3]ssim=shortest=1"
    $output = & $ffmpeg -hide_banner -i $Distorted -i $Reference -lavfi $graph -f null - 2>&1 | Out-String
    $vmaf = if ($output -match 'VMAF score: ([\d.]+)') { [double]$Matches[1] } else { [double]::NaN }
    $psnr = if ($output -match 'PSNR y:[\d.]+ u:[\d.]+ v:[\d.]+ average:([\d.]+)') { [double]$Matches[1] } else { [double]::NaN }
    $ssim = if ($output -match 'SSIM Y:[\d.]+ .* All:([\d.]+)') { [double]$Matches[1] } else { [double]::NaN }
    [pscustomobject]@{ Vmaf = $vmaf; Psnr = $psnr; Ssim = $ssim }
}

Add-Type -Namespace PalaClips -Name Power -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);'
$continuous = [uint32]2147483648
[PalaClips.Power]::SetThreadExecutionState($continuous -bor 1) | Out-Null
try {
    if (-not (Test-Path $Source)) { throw "source introuvable : $Source" }
    $cpuName = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
    Write-Journal "$(Get-Date -Format s) $Name : $Seconds s de « $(Split-Path -Leaf $Source) » à partir de $StartSeconds s, $cpuName, $logical processeurs logiques"
    Write-Journal "Débit = préréglage Équilibré (0,18 bit/pixel). cœurs = temps CPU / durée de la vidéo (cœurs occupés en temps réel) ; machine = cœurs / $logical ; vitesse = durée / temps d'encodage sans cadence imposée (< 1 = ne suit pas le temps réel). Décodage de la référence soustrait. Qualité : VMAF NEG (insensible à l'accentuation), PSNR moyen, SSIM."
    foreach ($format in $Formats) {
        if ($format -notmatch '^(\d+)p(\d+)$') { throw "format invalide : $format (ex. 1080p60)" }
        $height = [int]$Matches[1]; $fps = [int]$Matches[2]
        $width = [int]([Math]::Round($height * 16 / 9 / 2) * 2)
        $kbps = [int][Math]::Round($width * $height * $fps * 0.18 / 1000)
        $reference = Join-Path $work "reference-$format.y4m"
        & $ffmpeg -hide_banner -loglevel error -y -ss $StartSeconds -t $Seconds -i $Source -an -vf "fps=$fps,scale=$($width):$($height):flags=lanczos,format=yuv420p" $reference
        if ($LASTEXITCODE -ne 0) { throw "extraction de la référence $format impossible" }
        $baseline = Invoke-Timed $reference @("-f", "null") "-"
        Write-Journal ""
        Write-Journal "== $format ($($width)×$height, $fps i/s, $([Math]::Round($kbps / 1000, 1)) Mb/s) — lecture de la référence : $([Math]::Round($baseline.Cpu, 2)) s CPU"
        Write-Journal ("{0,-34} {1,6} {2,8} {3,8} {4,9} {5,8} {6,8} {7,8}" -f "encodeur", "cœurs", "machine", "vitesse", "VMAF NEG", "PSNR", "SSIM", "Mb/s")
        $encoders = Get-Encoders $fps $kbps
        foreach ($label in $encoders.Keys) {
            $encoded = Join-Path $work "encoded.mp4"
            Remove-Item $encoded -ErrorAction SilentlyContinue
            $run = Invoke-Timed $reference $encoders[$label] $encoded
            if ($run.ExitCode -ne 0 -or -not (Test-Path $encoded)) {
                $reason = "$($run.Errors)".Trim() -split "`r?`n" | Select-Object -First 1
                Write-Journal ("{0,-34} ÉCHEC : {1}" -f $label, $reason)
                continue
            }
            $cores = [Math]::Max(0, $run.Cpu - $baseline.Cpu) / $Seconds
            $speed = $Seconds / $run.Wall
            $quality = Measure-Quality $encoded $reference $fps
            $mbps = (Get-Item $encoded).Length * 8 / $Seconds / 1e6
            Write-Journal ("{0,-34} {1,6:N2} {2,7:N1} % {3,7:N2}x {4,9:N2} {5,8:N2} {6,8:N4} {7,8:N1}" -f $label, $cores, ($cores / $logical * 100), $speed, $quality.Vmaf, $quality.Psnr, $quality.Ssim, $mbps)
        }
        Remove-Item $reference -ErrorAction SilentlyContinue
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    [PalaClips.Power]::SetThreadExecutionState($continuous) | Out-Null
    Write-Journal "$(Get-Date -Format s) fin : vidéos supprimées, journal $journal"
}
