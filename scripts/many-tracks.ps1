param(
    [int]$Apps = 24,
    [string]$Device = "Realtek",
    [string]$Name = "many-tracks",
    [switch]$Keep
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$cli = Join-Path $root "target\release\palaclips-cli.exe"
$work = Join-Path $root "target\$Name"
$clips = Join-Path $work "clips"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

$tones = @()
$process = $null
try {
    if (-not (Test-Path $cli)) { throw "compile d'abord : cargo build --release -p palaclips-cli" }
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $clips | Out-Null
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $appArgs = @()
    foreach ($index in 1..$Apps) {
        $exe = Join-Path $work ("tone-{0:D2}.exe" -f $index)
        Copy-Item $cli $exe
        $tones += Start-Process $exe -ArgumentList "play-tone", "--hz", "$(200 + 37 * $index)", "--amplitude", "0.05", "--seconds", "120", "--device", $Device -PassThru -WindowStyle Hidden
        $appArgs += @("--app", ("tone-{0:D2}.exe" -f $index))
    }
    Start-Sleep -Seconds 2
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = (@("replay", "--target", "primary", "--height", "720", "--fps", "30", "--bitrate-mbps", "4", "--duration", "30", "--no-game", "--tracks-mix", "--clips-dir", "`"$clips`"") + $appArgs) -join " "
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $process = [System.Diagnostics.Process]::Start($info)
    $output = $process.StandardOutput.ReadToEndAsync()
    $errors = $process.StandardError.ReadToEndAsync()
    Write-Journal "$(Get-Date -Format s) $Name : replay 720p30 avec Mix (somme), micro et $Apps applis"
    Start-Sleep -Seconds 20
    $process.StandardInput.WriteLine("")
    Start-Sleep -Seconds 4
    $process.StandardInput.WriteLine("q")
    if (-not $process.WaitForExit(60000)) { throw "la CLI ne s'arrête pas" }
    $clip = Get-ChildItem $clips -Recurse -Filter *.mp4 | Select-Object -First 1
    if (-not $clip) { throw "aucun clip : $($errors.Result)" }
    $titles = @(ffprobe -v error -select_streams a -show_entries stream_tags=title -of csv=p=0 $clip.FullName)
    Write-Journal ("clip : {0} pistes audio, {1:N1} Mo ; titres : {2}" -f $titles.Count, ($clip.Length / 1MB), ($titles -join ", "))
    $levels = foreach ($track in 0, 1, 2, ($titles.Count - 1)) {
        $peak = (ffmpeg -hide_banner -nostats -i $clip.FullName -map "0:a:$track" -af astats=metadata=0 -f null - 2>&1 | Select-String "Peak level dB" | Select-Object -Last 1).Line -replace '^.*: ', ''
        "piste $($track + 1) : crête $peak dB"
    }
    Write-Journal ($levels -join " ; ")
    $continuity = (python (Join-Path $PSScriptRoot "clip-continuity.py") $clip.FullName) -join " "
    Write-Journal ("continuité : {0}" -f ($continuity -replace '.*; ', ''))
    $compressed = Join-Path $work "compressed.mp4"
    & $cli compress $clip.FullName --target-mb 20 --all-tracks --out $compressed 2>&1 | Out-Null
    $kept = @(ffprobe -v error -select_streams a -show_entries stream=index -of csv=p=0 $compressed).Count
    Write-Journal ("compression « Garder toutes les pistes » : {0} pistes, {1:N2} Mo (cible 20 Mo = 19,07 Mio)" -f $kept, ((Get-Item $compressed).Length / 1MB))
    if ($Keep) { Copy-Item $clip.FullName (Join-Path $root "target\many-tracks-clip.mp4") -Force }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    if ($process -and -not $process.HasExited) { $process.Kill() }
    $tones | Where-Object { -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : sons arrêtés, fichiers de travail supprimés, journal $journal"
}
