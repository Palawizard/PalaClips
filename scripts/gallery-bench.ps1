param([int]$Clips = 5000, [int]$Port = 9339)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$free = (Get-PSDrive (Split-Path -Qualifier $root).TrimEnd(':')).Free
if ($free -lt 12GB) { throw "Moins de 12 Go libres sur $(Split-Path -Qualifier $root) : test annulé." }

$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "gallery-bench-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
function Write-Journal([string]$Line) { $Line | Tee-Object -FilePath $journal -Append | Write-Host }

$sources = Join-Path $root "target\bench-sources"
$fixtures = Join-Path $root "target\gallery-fixture-sources"
$long = Join-Path $sources "Fortnite_2026-01-01_00-00-00.mp4"
$localCache = Join-Path $env:LOCALAPPDATA "com.palawi.palaclips\cache"
$localBefore = @(Get-ChildItem $localCache -Recurse -File -ErrorAction SilentlyContinue).Count
$app = $null

try {
    New-Item -ItemType Directory -Force $sources | Out-Null
    $ffmpeg = Join-Path $root "third_party\ffmpeg\bin\ffmpeg.exe"
    $titles = @("Mix complet", "Micro", "Fortnite", "Discord", "Spotify", "Chrome")
    $inputs = @("-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=30:duration=3600")
    $maps = @("-map", "0:v")
    $metadata = @()
    for ($index = 0; $index -lt $titles.Count; $index++) {
        $inputs += @("-f", "lavfi", "-i", "sine=frequency=$(220 + 110 * $index):beep_factor=4:duration=3600:sample_rate=48000")
        $maps += @("-map", "$($index + 1):a")
        $metadata += @("-metadata:s:a:$index", "title=$($titles[$index])", "-metadata:s:a:$index", "handler_name=$($titles[$index])")
    }
    $generated = Measure-Command {
        & $ffmpeg -hide_banner -loglevel error -y @inputs @maps `
            -c:v h264_nvenc -rc cbr -cbr_padding 1 -b:v 9M -g 60 -bf 0 -c:a aac -b:a 192k -ac 2 `
            @metadata -movflags +hybrid_fragmented $long
    }
    if ($LASTEXITCODE -ne 0) { throw "ffmpeg n'a pas pu créer $long" }
    (Get-Item $long).LastWriteTime = Get-Date
    Write-Journal "$(Get-Date -Format s) enregistrement factice : 1 h, 6 pistes, $([math]::Round((Get-Item $long).Length / 1GB, 2)) Go, généré en $([math]::Round($generated.TotalSeconds)) s"

    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $app = & (Join-Path $PSScriptRoot "app-isolated.ps1") -Clips $Clips -Link $long -MainScreen `
        -Fixture @{ Seconds = 2; Size = "640x360"; VideoBitrate = "1M" } | Select-Object -Last 1
    $config = Join-Path $root "target\test-config"
    Write-Journal "$Clips clips factices + l'enregistrement d'1 h dans $config ; app isolée sur l'écran principal (PID $($app.Id))"

    node (Join-Path $PSScriptRoot "gallery-bench.mjs") $Port $app.Id $config (Split-Path $long -Leaf) ($Clips + 1) 2>&1 |
        ForEach-Object { Write-Journal "$_" }
    $passed = $LASTEXITCODE -eq 0
}
finally {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $root "target\test-config"), $sources, $fixtures -Recurse -Force -ErrorAction SilentlyContinue
    $localAfter = @(Get-ChildItem $localCache -Recurse -File -ErrorAction SilentlyContinue).Count
    Write-Journal "fichiers de test supprimés ; cache de %LOCALAPPDATA% : $localBefore fichiers avant, $localAfter après"
    Write-Host "journal : $journal"
}
if (-not $passed) { exit 1 }
