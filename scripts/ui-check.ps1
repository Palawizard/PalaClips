param(
    [int]$Port = 9335,
    [switch]$LeftScreen,
    [switch]$MainScreen,
    [switch]$RightScreen,
    [string]$Name = "ui-check"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$passed = $false

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

try {
    $screen = @{ LeftScreen = $LeftScreen; MainScreen = $MainScreen; RightScreen = $RightScreen }
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $settings = '{ "onboarded": true, "engine": { "replay": { "arm_on_startup": false }, "audio": { "desktop_in_full_mix": true } } }'
    $app = & (Join-Path $PSScriptRoot "app-isolated.ps1") -Settings $settings -Clips 3 @screen `
        -Fixture @{ Seconds = 12; Size = "640x360"; VideoBitrate = "1M" } | Select-Object -Last 1
    $config = Join-Path $root "target\test-config"
    Write-Journal "$(Get-Date -Format s) $Name : app isolée (PID $($app.Id)), replay désarmé, 3 clips factices, mode bureau dans $config"
    $shots = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm")"; node (Join-Path $PSScriptRoot "ui-check.mjs") $Port $shots 2>&1 | ForEach-Object { Write-Journal "$_" }
    $passed = $LASTEXITCODE -eq 0
}
finally {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $root "target\test-config") -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "app fermée, dossier de test supprimé ; $(if ($passed) { 'RÉUSSI' } else { 'ÉCHEC' }) ; journal : $journal"
}
