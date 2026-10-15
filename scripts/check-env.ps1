<#
.SYNOPSIS
    Vérifie que la machine de dev a tout ce qu'il faut pour PalaClips.
.EXAMPLE
    pwsh scripts/check-env.ps1
#>
$ErrorActionPreference = "Continue"
$ok = $true

function Check($label, [scriptblock]$test, $fix) {
    try {
        $result = & $test
        if ($result) {
            Write-Host ("[OK]   {0} : {1}" -f $label, $result) -ForegroundColor Green
            return
        }
    } catch { }
    Write-Host ("[KO]   {0}  ->  {1}" -f $label, $fix) -ForegroundColor Red
    $script:ok = $false
}

$root = Split-Path -Parent $PSScriptRoot

Check "Windows build >= 20348 (capture audio par appli)" {
    $b = [int](Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion").CurrentBuildNumber
    if ($b -ge 20348) { "build $b" }
} "Mettre Windows à jour (Windows 11 recommandé)"

Check "Rust (rustc)" { (rustc --version) } "Installer rustup : https://rustup.rs"
Check "Cible MSVC" { if ((rustup show active-toolchain) -match "msvc") { "msvc" } } "rustup default stable-x86_64-pc-windows-msvc"
Check "Visual Studio Build Tools (cl.exe)" {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) { & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName }
} "winget install Microsoft.VisualStudio.2022.BuildTools (charge de travail 'Desktop C++')"
Check "LLVM / libclang (bindgen)" { (clang --version | Select-Object -First 1) } "winget install LLVM.LLVM, puis rouvrir le terminal"
Check "Node.js >= 20.19" { (node --version) } "winget install OpenJS.NodeJS.LTS"
Check "WebView2 Runtime" {
    $k = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    if (Test-Path $k) { (Get-ItemProperty $k).pv }
} "Installé avec Windows 11 ; sinon https://developer.microsoft.com/microsoft-edge/webview2"
Check "GPU NVIDIA (NVENC)" { (nvidia-smi --query-gpu=name,driver_version --format=csv,noheader) } "Facultatif si GPU AMD/Intel ; sinon installer le pilote NVIDIA"
Check "FFmpeg LGPL (third_party/ffmpeg)" {
    if (Test-Path (Join-Path $root "third_party/ffmpeg/bin/avcodec-*.dll")) { "présent" }
} "pwsh scripts/fetch-ffmpeg.ps1"
Check "Icônes Tauri générées" {
    if (Test-Path (Join-Path $root "app/src-tauri/icons/icon.ico")) { "présentes" }
} "cd app; npm run tauri icon src-tauri/icons/app-icon.svg"
Check "Skill impeccable" {
    if ((Test-Path "$env:USERPROFILE\.claude\plugins") -or (Test-Path (Join-Path $root ".claude/skills/impeccable"))) { "trouvé (à confirmer avec /plugin)" }
} "Dans Claude Code : /plugin marketplace add pbakaus/impeccable puis /plugin -> installer impeccable"
Check "Skill emil-design-eng" {
    if ((Test-Path "$env:USERPROFILE\.claude\skills\emil-design-eng") -or (Test-Path (Join-Path $root ".claude/skills/emil-design-eng"))) { "présent" }
} "npx -y skills add emilkowalski/skill --skill emil-design-eng --agent claude-code"

Write-Host ""
if ($ok) { Write-Host "Environnement prêt." -ForegroundColor Green } else { Write-Host "Corrige les points [KO] ci-dessus." -ForegroundColor Yellow; exit 1 }
