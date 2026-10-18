param(
    [Parameter(Mandatory)][string]$Root,
    [int]$Count = 60,
    [int]$Seconds = 12
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$ffmpeg = Join-Path $repo "third_party\ffmpeg\bin\ffmpeg.exe"
$Root = [IO.Path]::GetFullPath($Root)
$allowed = [IO.Path]::GetFullPath((Join-Path $repo "target"))
if (-not $Root.StartsWith($allowed, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Les clips factices ne s'écrivent que sous $allowed."
}
$sources = Join-Path $repo "target\gallery-fixture-sources"
New-Item -ItemType Directory -Force $sources, $Root | Out-Null

$variants = @(
    @{ Name = "a"; Video = "testsrc2=size=1280x720:rate=60"; Tones = @(220, 330, 495) },
    @{ Name = "b"; Video = "testsrc=size=1280x720:rate=60"; Tones = @(262, 392, 523) },
    @{ Name = "c"; Video = "smptehdbars=size=1280x720:rate=60"; Tones = @(196, 294, 440) }
)
foreach ($variant in $variants) {
    $file = Join-Path $sources "$($variant.Name).mp4"
    if (Test-Path $file) { continue }
    $inputs = @("-f", "lavfi", "-i", "$($variant.Video):duration=$Seconds")
    foreach ($tone in $variant.Tones) {
        $inputs += @("-f", "lavfi", "-i", "sine=frequency=$($tone):beep_factor=4:duration=$($Seconds):sample_rate=48000")
    }
    $maps = @("-map", "0:v")
    for ($index = 1; $index -le $variant.Tones.Count; $index++) { $maps += @("-map", "$($index):a") }
    & $ffmpeg -hide_banner -loglevel error -y @inputs @maps `
        -c:v h264_nvenc -b:v 6M -g 60 -bf 0 -c:a aac -b:a 192k -ac 2 `
        -metadata:s:a:0 title="Mix complet" -metadata:s:a:0 handler_name="Mix complet" `
        -metadata:s:a:1 title="Micro" -metadata:s:a:1 handler_name="Micro" `
        -metadata:s:a:2 title="Discord" -metadata:s:a:2 handler_name="Discord" `
        $file
    if ($LASTEXITCODE -ne 0) { throw "ffmpeg n'a pas pu créer $file" }
}

$games = @("Fortnite", "Valorant", "Rocket League", "Bureau")
$now = Get-Date
for ($index = 0; $index -lt $Count; $index++) {
    $game = $games[$index % $games.Count]
    $when = $now.AddMinutes(-37 * $index)
    $stamp = $when.ToString("yyyy-MM-dd_HH-mm-ss")
    $suffix = if ($index % 5 -eq 4) { "" } else { "_replay" }
    $dir = Join-Path $Root $game
    New-Item -ItemType Directory -Force $dir | Out-Null
    $target = Join-Path $dir "$($game)_$($stamp)$($suffix)_$index.mp4"
    Copy-Item (Join-Path $sources "$($variants[$index % $variants.Count].Name).mp4") $target
    (Get-Item $target).LastWriteTime = $when
}
Write-Host "$Count clips factices dans $Root"
