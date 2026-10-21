param(
    [string]$Tag = "test-v1.0.1",
    [int]$Port = 8765,
    [int]$DebugPort = 9347,
    [string]$Name = "update-test"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$work = Join-Path $root "target\update-test"
$installDir = Join-Path $env:LOCALAPPDATA "PalaClips"
$installedExe = Join-Path $installDir "palaclips-app.exe"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$uninstallKeys = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Stop-Apps {
    Get-Process palaclips-app -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -like "$root\target\*" -or $_.Path -eq $installedExe } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

function Write-Feed([string]$File, [string]$Version, [string]$Signature, [string]$Installer) {
    @{
        version   = $Version
        notes     = "Version de test $Version, jamais publiée."
        pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        platforms = @{ "windows-x86_64" = @{ signature = $Signature; url = "http://127.0.0.1:$Port/$Installer" } }
    } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $work $File) -Encoding utf8
}

function Start-TestApp([string]$Feed) {
    $env:PALACLIPS_UPDATE_URL = "http://127.0.0.1:$Port/$Feed"
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$DebugPort"
    & (Join-Path $PSScriptRoot "app-isolated.ps1") -MainScreen | Out-Null
}

function Get-DataFingerprint {
    $fingerprint = [ordered]@{}
    foreach ($dir in @((Join-Path $env:APPDATA "com.palawi.palaclips"), (Join-Path $env:LOCALAPPDATA "com.palawi.palaclips"))) {
        if (-not (Test-Path $dir)) { $fingerprint[$dir] = "absent"; continue }
        foreach ($file in Get-ChildItem $dir -Recurse -File -Force -ErrorAction SilentlyContinue) {
            $fingerprint[$file.FullName] = "$($file.Length) $($file.LastWriteTimeUtc.Ticks)"
        }
    }
    $fingerprint
}

function Get-UninstallEntry {
    Get-ItemProperty $uninstallKeys -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -like "PalaClips*" }
}

$server = $null
$before = $null
$emptyInstallDir = $false
$passed = $false
try {
    if (Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in @("palaclips-app", "PalaClips") }) {
        throw "PalaClips tourne : quitte-le avant l'essai"
    }
    if ((Test-Path $installDir) -and @(Get-ChildItem $installDir -Force).Count -gt 0 -or (Get-UninstallEntry)) {
        throw "une installation de PalaClips existe déjà ($installDir) : l'essai ne l'écrase pas"
    }
    $emptyInstallDir = Test-Path $installDir
    $before = Get-DataFingerprint
    Write-Journal "données de Palawi avant : $($before.Count) entrées (taille et date de chaque fichier)"

    Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $work | Out-Null
    gh release download $Tag --pattern "*setup.exe*" --dir $work
    if ($LASTEXITCODE -ne 0) { throw "brouillon $Tag introuvable" }
    $installer = Get-ChildItem $work -Filter "*setup.exe" | Select-Object -First 1
    $signature = (Get-Content "$($installer.FullName).sig" -Raw).Trim()
    $decoded = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($signature)) -split "`n"
    $decoded[1] = $decoded[1].Substring(0, 10) + $(if ($decoded[1][10] -eq "A") { "B" } else { "A" }) + $decoded[1].Substring(11)
    $tampered = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($decoded -join "`n"))
    Write-Feed "latest.json" "1.0.1" $signature $installer.Name
    Write-Feed "latest-bad.json" "1.0.2" $tampered $installer.Name
    $python = (Get-Command python).Source
    $server = Start-Process $python -ArgumentList @("-m", "http.server", $Port, "--bind", "127.0.0.1", "-d", $work) -WindowStyle Hidden -PassThru
    Write-Journal "$(Get-Date -Format s) $Name : installeur $($installer.Name) du brouillon $Tag, serveur local 127.0.0.1:$Port"

    Push-Location (Join-Path $root "app")
    $versionConfig = Join-Path $work "version-1.0.0.json"
    '{"version":"1.0.0","plugins":{"updater":{"dangerousInsecureTransportProtocol":true}}}' | Set-Content $versionConfig -Encoding ascii
    npx tauri build --no-bundle --config $versionConfig 2>&1 | Select-Object -Last 1 | ForEach-Object { Write-Journal "build 1.0.0 : $_" }
    Pop-Location
    if ($LASTEXITCODE -ne 0) { throw "build 1.0.0 impossible" }

    Start-TestApp "latest-bad.json"
    node (Join-Path $PSScriptRoot "update-test.mjs") $DebugPort bad 2>&1 | ForEach-Object { Write-Journal "signature fausse : $_" }
    $badRefused = $LASTEXITCODE -eq 0
    Write-Journal "signature fausse refusée, app restée en 1.0.0 : $badRefused"
    Stop-Apps

    Start-TestApp "latest.json"
    node (Join-Path $PSScriptRoot "update-test.mjs") $DebugPort good 2>&1 | ForEach-Object { Write-Journal "bonne signature : $_" }
    $deadline = (Get-Date).AddSeconds(180)
    while (-not ((Get-UninstallEntry).DisplayVersion -eq "1.0.1" -and (Test-Path $installedExe))) {
        if ((Get-Date) -gt $deadline) { throw "l'installeur 1.0.1 n'a pas terminé" }
        Start-Sleep -Seconds 1
    }
    Start-Sleep -Seconds 3
    $relaunched = @(Get-Process palaclips-app -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installedExe }).Count
    Write-Journal "relance automatique par l'installeur (désactivée en mode isolé) : $relaunched processus"
    Start-Process $installedExe | Out-Null
    Write-Journal "app installée en $((Get-UninstallEntry).DisplayVersion) dans $installDir, relancée en mode isolé"
    node (Join-Path $PSScriptRoot "update-test.mjs") $DebugPort installed 2>&1 | ForEach-Object { Write-Journal "après mise à jour : $_" }
    $updated = $LASTEXITCODE -eq 0
    $starts = @(Get-Content (Join-Path $root "target\test-config\local\logs\*.log") | Select-String "démarrage de PalaClips").Count
    Write-Journal "journal isolé : $starts démarrage(s) dans target\test-config (l'app 1.0.0 puis l'app installée ; attendu 2)"
    $passed = $badRefused -and $updated -and $starts -ge 2 -and $relaunched -eq 0
}
catch {
    Write-Journal "ÉCHEC : $_"
}
finally {
    Stop-Apps
    $uninstaller = Join-Path $installDir "uninstall.exe"
    if (Test-Path $uninstaller) {
        Start-Process $uninstaller -ArgumentList "/S" -Wait
        $deadline = (Get-Date).AddSeconds(60)
        while ((Test-Path $installedExe) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 1 }
    }
    if ($emptyInstallDir -and -not (Test-Path $installDir)) { New-Item -ItemType Directory $installDir | Out-Null }
    $shortcut = Get-ChildItem ([Environment]::GetFolderPath("Programs")) -Recurse -Filter "PalaClips*.lnk" -ErrorAction SilentlyContinue
    $leftover = (Test-Path $installDir) -and @(Get-ChildItem $installDir -Force).Count -gt 0
    Write-Journal "désinstallation : dossier $(if ($leftover) { 'ENCORE PRÉSENT' } elseif ($emptyInstallDir) { 'vidé (il existait vide avant l''essai)' } else { 'supprimé' }), clé $(if (Get-UninstallEntry) { 'ENCORE PRÉSENTE' } else { 'supprimée' }), raccourci $(if ($shortcut) { 'ENCORE PRÉSENT' } else { 'supprimé' })"
    if ($server -and -not $server.HasExited) { Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item Env:PALACLIPS_UPDATE_URL, Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS, Env:PALACLIPS_CONFIG_DIR, Env:PALACLIPS_TEST_WINDOW -ErrorAction SilentlyContinue
    Remove-Item $work, (Join-Path $root "target\test-config") -Recurse -Force -ErrorAction SilentlyContinue
    if ($before) {
        $after = Get-DataFingerprint
        $changed = @($before.Keys | Where-Object { $after[$_] -ne $before[$_] }) + @($after.Keys | Where-Object { -not $before.Contains($_) })
        Write-Journal "données de Palawi après : $(if ($changed) { "MODIFIÉES : $($changed -join ' ; ')" } else { "$($after.Count) entrées identiques" })"
        if ($changed) { $passed = $false }
    }
    Write-Journal "RÉSULTAT : $(if ($passed) { 'mise à jour 1.0.0 → 1.0.1 installée, fausse signature refusée' } else { 'ÉCHEC' })"
    Write-Journal "$(Get-Date -Format s) fin : apps arrêtées, serveur arrêté, dossiers de test supprimés ; supprimer ensuite le brouillon : gh release delete $Tag --cleanup-tag -y"
}
