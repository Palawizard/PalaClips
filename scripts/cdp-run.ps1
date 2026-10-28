param([Parameter(Mandatory)][string]$Script, [string]$Settings = '{ "onboarded": true, "engine": { "replay": { "arm_on_startup": false } } }', [int]$Clips = 0, [string[]]$Copy = @(), [string[]]$Link = @(), [int]$Port = 9336, [string[]]$Extra = @(), [string]$ClipsDir = '')
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root ("target\journals\" + [IO.Path]::GetFileNameWithoutExtension($Script))
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
try {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $launch = @{ Settings = $Settings; MainScreen = $true; Clips = $Clips; Copy = $Copy; Link = $Link; ClipsDir = $ClipsDir }
    if ($Clips -gt 0) { $launch.Fixture = @{ Seconds = 12; Size = "640x360"; VideoBitrate = "1M" } }
    & (Join-Path $root "scripts\app-isolated.ps1") @launch | Out-Null
    node (Resolve-Path $Script) $Port $out @Extra
}
finally {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $root "target\test-config") -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "app fermée, dossier de test supprimé"
}
