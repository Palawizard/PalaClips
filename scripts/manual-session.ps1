param([string]$Root = "F:\PalaClips-tests\manual", [string]$LongClip = "F:\PalaClips-tests\scrub\Test\Test_2026-10-27_12-00-00_replay.mp4", [switch]$Stop)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$ffmpeg = Join-Path $repo "third_party\ffmpeg\bin\ffmpeg.exe"

if ($Stop) {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$repo\target\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Remove-Item Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $repo "target\test-config") -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
    "app de test fermée, $Root et target\test-config supprimés"
    return
}

Remove-Item $Root -Recurse -Force -ErrorAction SilentlyContinue
$games = @("Fortnite", "Valorant", "Rocket League")
for ($index = 0; $index -lt 14; $index++) {
    $game = $games[$index % 3]
    $stamp = (Get-Date).AddMinutes(-10 * ($index + 1))
    $dir = Join-Path $Root $game
    New-Item -ItemType Directory -Force $dir | Out-Null
    $file = Join-Path $dir "$($game)_$($stamp.ToString('yyyy-MM-dd_HH-mm-ss'))_replay.mp4"
    & $ffmpeg -hide_banner -loglevel error -y -f lavfi -i "testsrc2=size=640x360:rate=60:duration=12" `
        -f lavfi -i "sine=frequency=220:duration=12" -f lavfi -i "sine=frequency=330:duration=12" -f lavfi -i "sine=frequency=495:duration=12" `
        -map 0 -map 1 -map 2 -map 3 -c:v h264_nvenc -bf 0 -b:v 1M -g 60 -c:a aac -b:a 96k `
        -metadata:s:a:0 title="Mix complet" -metadata:s:a:0 handler_name="Mix complet" -metadata:s:a:1 title="Micro" `
        -metadata:s:a:1 handler_name="Micro" -metadata:s:a:2 title="Discord" -metadata:s:a:2 handler_name="Discord" $file
    (Get-Item $file).LastWriteTime = $stamp
}
$many = Join-Path $Root "Pistes"
New-Item -ItemType Directory -Force $many | Out-Null
$titles = @("Mix complet", "Micro", "Jeu", "Discord", "Spotify", "Firefox", "OBS", "VLC")
$inputs = @("-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=60:duration=40")
$maps = @("-map", "0")
for ($track = 0; $track -lt $titles.Count; $track++) {
    $inputs += @("-f", "lavfi", "-i", "sine=frequency=$(200 + 60 * $track):beep_factor=$($track + 1):duration=40")
    $maps += @("-map", "$($track + 1)", "-metadata:s:a:$track", "title=$($titles[$track])", "-metadata:s:a:$track", "handler_name=$($titles[$track])")
}
& $ffmpeg -hide_banner -loglevel error -y @inputs @maps -c:v h264_nvenc -bf 0 -b:v 3M -g 60 -c:a aac -b:a 96k (Join-Path $many "Pistes_$((Get-Date).AddMinutes(-1).ToString('yyyy-MM-dd_HH-mm-ss'))_replay.mp4")
$long = Join-Path $Root "Test"
New-Item -ItemType Directory -Force $long | Out-Null
New-Item -ItemType HardLink -Path (Join-Path $long (Split-Path $LongClip -Leaf)) -Target $LongClip | Out-Null

$settings = '{ "onboarded": true, "engine": { "capture": { "kind": "primary_monitor" }, "replay": { "arm_on_startup": true, "duration_secs": 30, "after_secs": 10 } } }'
& (Join-Path $PSScriptRoot "app-isolated.ps1") -MainScreen -ClipsDir $Root -Settings $settings | Out-Null
"app de test ouverte, clips dans $Root"
