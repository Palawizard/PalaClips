param(
    [Parameter(Mandatory)][ValidateSet("Create", "Stage", "Remove")][string]$Step,
    [string]$Account = "PalaClipsTest",
    [int]$Port = 8765
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$public = Join-Path $env:PUBLIC "PalaClips-test"
$updates = Join-Path $root "target\update-test"
$usersGroup = "S-1-5-32-545"

function Assert-Admin {
    $identity = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $identity.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw "Lance cette étape dans un PowerShell administrateur."
    }
}

function Write-Launcher([string]$Name, [string]$Feed) {
    @"
@echo off
set PALACLIPS_UPDATE_URL=http://127.0.0.1:$Port/$Feed
start "" "%LOCALAPPDATA%\PalaClips\palaclips-app.exe"
"@ | Set-Content (Join-Path $public $Name) -Encoding ascii
}

function Write-Feed([string]$Name, [string]$Version, [string]$Signature, [string]$Installer) {
    @{
        version   = $Version
        notes     = "Version de test $Version (essai de mise à jour, jamais publiée)."
        pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        platforms = @{
            "windows-x86_64" = @{
                signature = $Signature
                url       = "http://127.0.0.1:$Port/$Installer"
            }
        }
    } | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $updates $Name) -Encoding utf8
}

switch ($Step) {
    "Create" {
        Assert-Admin
        $password = Read-Host -AsSecureString "Mot de passe du compte $Account"
        New-LocalUser -Name $Account -Password $password -FullName "PalaClips Test" `
            -Description "Compte temporaire pour tester PalaClips, à supprimer" -AccountNeverExpires | Out-Null
        Add-LocalGroupMember -SID $usersGroup -Member $Account
        Write-Host "Compte $Account créé (utilisateur standard)."
    }
    "Stage" {
        Remove-Item $updates, $public -Recurse -Force -ErrorAction SilentlyContinue
        New-Item -ItemType Directory -Force $updates, $public | Out-Null
        foreach ($version in "1.0.0", "1.0.1") {
            gh release download "test-v$version" --pattern "*setup.exe*" --dir (Join-Path $updates $version)
            if ($LASTEXITCODE -ne 0) { throw "brouillon test-v$version introuvable" }
        }
        $old = Get-ChildItem (Join-Path $updates "1.0.0") -Filter "*setup.exe" | Select-Object -First 1
        $new = Get-ChildItem (Join-Path $updates "1.0.1") -Filter "*setup.exe" | Select-Object -First 1
        $newSignature = (Get-Content "$($new.FullName).sig" -Raw).Trim()
        $oldSignature = (Get-Content "$($old.FullName).sig" -Raw).Trim()
        Write-Feed "latest.json" "1.0.1" $newSignature "1.0.1/$($new.Name)"
        Write-Feed "latest-bad.json" "1.0.2" $oldSignature "1.0.1/$($new.Name)"
        Copy-Item $old.FullName $public
        Write-Launcher "1-lancer-avec-maj-locale.cmd" "latest.json"
        Write-Launcher "2-lancer-avec-signature-fausse.cmd" "latest-bad.json"
        @"
@echo off
xcopy "%LOCALAPPDATA%\com.palawi.palaclips\logs" "%PUBLIC%\PalaClips-test\logs\" /Y /I
"@ | Set-Content (Join-Path $public "3-copier-journaux.cmd") -Encoding ascii
        $python = (Get-Command python).Source
        $server = Start-Process $python -ArgumentList @("-m", "http.server", $Port, "--bind", "127.0.0.1", "-d", $updates) `
            -WindowStyle Hidden -PassThru
        $server.Id | Set-Content (Join-Path $updates "server.pid")
        Write-Host "Prêt : $public (installeur 1.0.0 + 2 lanceurs) ; serveur local PID $($server.Id) sur 127.0.0.1:$Port"
    }
    "Remove" {
        Assert-Admin
        $ErrorActionPreference = "Continue"
        $user = Get-LocalUser -Name $Account -ErrorAction SilentlyContinue
        if ($user) {
            foreach ($line in (quser 2>$null)) {
                if ($line -match "^\s*>?$([regex]::Escape($Account))\s+(?:\S+\s+)?(\d+)\s") {
                    logoff $Matches[1]
                    Write-Host "Session $Account fermée."
                    Start-Sleep -Seconds 5
                }
            }
            try {
                Get-CimInstance Win32_UserProfile | Where-Object { $_.SID -eq $user.SID.Value } | Remove-CimInstance -ErrorAction Stop
                Write-Host "Profil de $Account supprimé."
            } catch {
                Write-Warning "Profil de $Account non supprimé ($_). Redémarre puis relance -Step Remove."
            }
            try {
                Remove-LocalUser -Name $Account -ErrorAction Stop
                Write-Host "Compte $Account supprimé."
            } catch {
                Write-Warning "Compte $Account non supprimé : $_"
            }
        } else {
            Write-Host "Aucun compte $Account."
        }
        $pidFile = Join-Path $updates "server.pid"
        if (Test-Path $pidFile) { Stop-Process -Id (Get-Content $pidFile) -Force -ErrorAction SilentlyContinue }
        Get-CimInstance Win32_Process -Filter "Name = 'python.exe'" |
            Where-Object { $_.CommandLine -match "http\.server $Port" } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        Remove-Item $public, $updates -Recurse -Force -ErrorAction SilentlyContinue
        $left = @($public, $updates) | Where-Object { Test-Path $_ }
        if ($left) { Write-Warning "Encore présent : $($left -join ', ')" } else { Write-Host "Serveur local arrêté, $public et $updates supprimés." }
    }
}
