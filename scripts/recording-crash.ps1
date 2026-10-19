param([int]$Port = 9340)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "recording-crash-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
function Write-Journal([string]$Line) { $Line | Tee-Object -FilePath $journal -Append | Write-Host }

$bin = Join-Path $root "third_party\ffmpeg\bin"
$config = Join-Path $root "target\test-config"
$clips = Join-Path $config "clips"
$cdp = Join-Path $PSScriptRoot "cdp.mjs"
$failures = @()
$ffplay = $null

function Invoke-Page([string]$Expression) {
    $output = node $cdp $Port $Expression
    if ($LASTEXITCODE -ne 0) { throw "CDP : $output" }
    $output | ConvertFrom-Json
}
function Get-Duration([string]$Path) {
    $value = & (Join-Path $bin "ffprobe.exe") -v error -show_entries format=duration -of csv=p=0 $Path
    if ($value) { [double]$value } else { 0 }
}
function Get-Recordings { @(Get-ChildItem $clips -Recurse -Filter *.mp4 | Sort-Object Name) }
function Stop-App {
    Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$root\target\*" } |
        ForEach-Object { taskkill /F /T /PID $_.Id | Out-Null }
    Start-Sleep -Seconds 2
}

try {
    Add-Type -AssemblyName System.Windows.Forms
    $area = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $ffplay = Start-Process (Join-Path $bin "ffplay.exe") -PassThru -ArgumentList @(
        "-hide_banner", "-loglevel", "error", "-an", "-x", "640", "-y", "360",
        "-left", "$($area.Right - 660)", "-top", "$($area.Bottom - 400)",
        "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=60"
    )
    $settings = @{
        engine = @{
            capture = @{ kind = "window"; exe_name = "ffplay.exe"; title_hint = $null }
            audio = @{ record_microphone = $false }
            record = @{ split = @{ kind = "duration"; minutes = 1 } }
        }
    } | ConvertTo-Json -Depth 10 -Compress
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
    $app = & (Join-Path $PSScriptRoot "app-isolated.ps1") -Settings $settings -MainScreen | Select-Object -Last 1
    Invoke-Page "1" | Out-Null
    Write-Journal "$(Get-Date -Format s) app isolée (PID $($app.Id)), capture de la fenêtre ffplay, découpage 1 min"

    Invoke-Page 'window.__TAURI_INTERNALS__.invoke("record_start")' | Out-Null
    Start-Sleep -Seconds 75
    Invoke-Page 'window.__TAURI_INTERNALS__.invoke("record_stop")' | Out-Null
    Start-Sleep -Seconds 5
    $split = Get-Recordings
    $total = 0
    foreach ($file in $split) {
        $duration = Get-Duration $file.FullName
        $total += $duration
        Write-Journal "découpage : $($file.Name) $([math]::Round($duration, 2)) s, $([math]::Round($file.Length / 1MB, 1)) Mo"
    }
    if ($split.Count -ne 2) { $failures += "$($split.Count) fichiers au lieu de 2 après 75 s" }
    if ([math]::Abs($total - 75) -gt 3) { $failures += "durée totale $([math]::Round($total, 1)) s au lieu de ~75 s" }
    $split | Remove-Item -Force

    Invoke-Page 'window.__TAURI_INTERNALS__.invoke("record_start")' | Out-Null
    Start-Sleep -Seconds 12
    Stop-App
    $crashed = Get-Recordings
    $pending = Join-Path $config "recordings.txt"
    Write-Journal "arrêt brutal après 12 s : $($crashed.Count) fichier(s), journal des enregistrements : $(if (Test-Path $pending) { (Get-Content $pending) -join ' | ' } else { 'absent' })"
    if ($crashed.Count -ne 1) { throw "$($crashed.Count) fichiers après l'arrêt brutal" }
    $before = Get-Duration $crashed[0].FullName
    Write-Journal "avant redémarrage : $($crashed[0].Name) lisible sur $([math]::Round($before, 2)) s"
    if ($before -lt 10) { $failures += "fichier interrompu lisible sur $before s seulement" }

    $app = Start-Process (Join-Path $root "target\release\palaclips-app.exe") -PassThru
    Invoke-Page "1" | Out-Null
    $deadline = (Get-Date).AddSeconds(30)
    while ((Test-Path $pending) -and (Get-Content $pending) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    Start-Sleep -Seconds 3
    $after = Get-Duration $crashed[0].FullName
    $clip = Invoke-Page "window.__TAURI_INTERNALS__.invoke('list_clips', { query: {} }).then((clips) => clips.map((clip) => ({ name: clip.name, kind: clip.kind, duration: clip.duration_secs })))"
    Write-Journal "après redémarrage : lisible sur $([math]::Round($after, 2)) s ; galerie : $($clip | ConvertTo-Json -Compress)"
    if ((Test-Path $pending) -and (Get-Content $pending)) { $failures += "journal des enregistrements non vidé" }
    if ($after -lt $before - 0.5) { $failures += "durée après finalisation $after s < $before s" }
    if (@($clip).Count -ne 1 -or $clip.kind -ne "recording" -or $clip.duration -lt 10) { $failures += "clip absent ou incomplet dans la galerie" }
}
catch {
    $failures += "$_"
}
finally {
    Stop-App
    if ($ffplay -and -not $ffplay.HasExited) { Stop-Process -Id $ffplay.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item $config -Recurse -Force -ErrorAction SilentlyContinue
    if ($failures.Count -eq 0) { Write-Journal "OK" } else { Write-Journal "ÉCHEC : $($failures -join ' ; ')" }
    Write-Host "journal : $journal"
}
if ($failures.Count -ne 0) { exit 1 }
