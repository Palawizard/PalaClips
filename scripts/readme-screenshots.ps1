param(
    [Parameter(Mandatory)][string[]]$Clip,
    [ValidateSet("fr", "en")][string]$Language = "fr",
    [string]$PlayerClip = "",
    [int]$Port = 9334
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "docs\screenshots\$Language"
New-Item -ItemType Directory -Force $out | Out-Null

function Stop-TestApp {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

$staging = Join-Path $root "target\readme-clips"
$ffmpeg = Join-Path $root "third_party\ffmpeg\bin\ffmpeg.exe"

try {
    Remove-Item $staging -Recurse -Force -ErrorAction SilentlyContinue
    $titles = if ($Language -eq "en") { @("Full mix", "Mic") } else { @("Mix complet", "Micro") }
    $staged = foreach ($file in $Clip) {
        $item = Get-Item -LiteralPath $file
        $game = $item.Directory.Name
        $stamp = $item.LastWriteTime.ToString("yyyy-MM-dd_HH-mm-ss")
        $target = Join-Path $staging "$game\$($game)_$($stamp)_replay.mp4"
        New-Item -ItemType Directory -Force (Split-Path $target) | Out-Null
        & $ffmpeg -hide_banner -loglevel error -y -i $item.FullName -map 0 -c copy `
            -metadata:s:a:0 title="$($titles[0])" -metadata:s:a:0 handler_name="$($titles[0])" `
            -metadata:s:a:1 title="$($titles[1])" -metadata:s:a:1 handler_name="$($titles[1])" $target
        if ($LASTEXITCODE -ne 0) { throw "remux impossible : $file" }
        (Get-Item $target).LastWriteTime = $item.LastWriteTime
        $target
    }

    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    & (Join-Path $PSScriptRoot "app-isolated.ps1") -MainScreen -Copy $staged -Settings "{`"language`":`"$Language`"}" | Out-Null
    node (Join-Path $PSScriptRoot "readme-screenshots.mjs") $Port $out app $Language $PlayerClip
    if ($LASTEXITCODE -ne 0) { throw "captures de l'app interrompues" }
    Stop-TestApp

    & (Join-Path $PSScriptRoot "app-isolated.ps1") -MainScreen -Settings "{`"onboarded`":false,`"language`":`"$Language`"}" | Out-Null
    node (Join-Path $PSScriptRoot "readme-screenshots.mjs") $Port $out onboarding $Language
    if ($LASTEXITCODE -ne 0) { throw "captures du premier lancement interrompues" }
}
finally {
    Stop-TestApp
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $root "target\test-config"), $staging -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "captures dans $out ; app de test arrêtée, dossier de test et copies des clips supprimés"
}
