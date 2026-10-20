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
} "Windows 11 requis (build 20348 ou plus récent)"

Check "Rust (rustc)" { (rustc --version) } "Installer rustup : https://rustup.rs"
Check "Cible MSVC" { if ((rustup show active-toolchain) -match "msvc") { "msvc" } } "rustup default stable-x86_64-pc-windows-msvc"
Check "Visual Studio Build Tools (cl.exe)" {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) { & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName }
} "winget install Microsoft.VisualStudio.2022.BuildTools (charge de travail 'Desktop C++')"
Check "LLVM / libclang (bindgen)" { (clang --version | Select-Object -First 1) } "winget install LLVM.LLVM, ajouter C:\Program Files\LLVM\bin au PATH, puis rouvrir le terminal"
Check "clang trouve MSVC et le Windows SDK" {
    "#include <errno.h>" | clang -E -x c - 2>$null | Out-Null
    if ($LASTEXITCODE -eq 0) { "errno.h trouvé" }
} "clang choisit une installation Visual Studio sans outils C++ : ajouter la charge 'Développement Desktop en C++' à cette installation ou la désinstaller"
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
    $installed = "$env:USERPROFILE\.claude\plugins\installed_plugins.json"
    if ((Test-Path $installed) -and (Select-String -Path $installed -Pattern '"impeccable@' -Quiet)) { "plugin installé" }
    elseif (Test-Path (Join-Path $root ".claude/skills/impeccable")) { "skill du projet" }
} "Dans Claude Code : /plugin marketplace add pbakaus/impeccable puis /plugin -> installer impeccable"
Check "Skill emil-design-eng" {
    $dirs = @("$env:USERPROFILE\.claude\skills", (Join-Path $root ".claude/skills")) | Where-Object { Test-Path $_ }
    if ($dirs | Get-ChildItem -Recurse -Depth 2 -Directory -Filter "emil-design-eng" -ErrorAction SilentlyContinue | Select-Object -First 1) { "présent" }
} "npx -y skills add emilkowalski/skill --skill emil-design-eng --agent claude-code"

Write-Host ""
if ($ok) { Write-Host "Environnement prêt." -ForegroundColor Green } else { Write-Host "Corrige les points [KO] ci-dessus." -ForegroundColor Yellow; exit 1 }
