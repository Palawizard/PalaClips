param([int]$Port = 9343)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$real = Join-Path $env:LOCALAPPDATA "com.palawi.palaclips\EBWebView\Default\Local Storage"
$isolated = Join-Path $root "target\test-config\webview"
$cdp = Join-Path $PSScriptRoot "cdp.mjs"

function Get-Fingerprint([string]$Dir) {
    if (-not (Test-Path $Dir)) { return "absent" }
    (Get-ChildItem $Dir -Recurse -File | Sort-Object FullName | ForEach-Object {
        "$($_.FullName.Substring($Dir.Length)) $($_.Length) $($_.LastWriteTimeUtc.Ticks) $((Get-FileHash $_.FullName -Algorithm SHA256).Hash)"
    }) -join "`n"
}

$before = Get-Fingerprint $real
$failures = @()
try {
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    & (Join-Path $PSScriptRoot "app-isolated.ps1") -Settings '{"engine":{"replay":{"arm_on_startup":false}}}' -MainScreen | Out-Null
    node $cdp $Port 'localStorage.setItem("palaclips.isolation.test", String(Date.now())); localStorage.getItem("palaclips.isolation.test")' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "CDP indisponible" }
    Start-Sleep -Seconds 2
}
catch {
    $failures += "$_"
}
finally {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } |
        ForEach-Object { taskkill /F /T /PID $_.Id | Out-Null }
    Start-Sleep -Seconds 2
}
$after = Get-Fingerprint $real
$isolatedStorage = Join-Path $isolated "EBWebView\Default\Local Storage"
if (-not (Test-Path $isolatedStorage)) { $failures += "pas de stockage WebView dans $isolated" }
if ($before -ne $after) { $failures += "le localStorage réel a changé" }
Remove-Item (Join-Path $root "target\test-config") -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
"stockage réel : $(if ($before -eq 'absent') { 'absent' } else { "$(@($before -split "`n").Count) fichiers" }), identique après le test : $($before -eq $after)"
if ($failures.Count -eq 0) { "OK" } else { "ÉCHEC : $($failures -join ' ; ')"; exit 1 }
