param([string]$Settings = '{}', [int]$Clips = 0, [hashtable]$Fixture = @{}, [string[]]$Link = @(), [string[]]$Copy = @(), [switch]$LeftScreen, [switch]$MainScreen, [switch]$RightScreen, [switch]$Tray)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$running = Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.ProcessName -in @('palaclips-app', 'PalaClips') }
if ($running) {
    throw "PalaClips est déjà ouvert ($(($running | ForEach-Object Path) -join ', ')) : l'instance unique enverrait le test vers lui, avec ses vrais réglages. Ferme-le d'abord (zone de notification > Quitter)."
}
$dir = Join-Path $root 'target\test-config'
if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
New-Item -ItemType Directory -Force (Join-Path $dir 'clips') | Out-Null
$json = $Settings | ConvertFrom-Json -AsHashtable
if (-not $json.schema_version) {
    $json.schema_version = [int]([regex]::Match((Get-Content (Join-Path $root 'app\src-tauri\src\settings.rs') -Raw), 'SETTINGS_VERSION: u32 = (\d+)').Groups[1].Value)
}
if (-not $json.engine) { $json.engine = @{} }
if (-not $json.engine.storage) { $json.engine.storage = @{} }
$json.engine.storage.clips_dir = Join-Path $dir 'clips'
$json | ConvertTo-Json -Depth 20 | Set-Content (Join-Path $dir 'settings.json') -Encoding utf8
if ($Clips -gt 0) { & (Join-Path $PSScriptRoot 'gallery-fixtures.ps1') -Root (Join-Path $dir 'clips') -Count $Clips @Fixture }
foreach ($file in $Link) {
    $name = Split-Path $file -Leaf
    $game = Join-Path $dir "clips\$($name.Split('_')[0])"
    New-Item -ItemType Directory -Force $game | Out-Null
    New-Item -ItemType HardLink -Path (Join-Path $game $name) -Target $file | Out-Null
}
foreach ($file in $Copy) {
    $item = Get-Item -LiteralPath $file
    $game = Join-Path $dir "clips\$($item.Directory.Name)"
    New-Item -ItemType Directory -Force $game | Out-Null
    Copy-Item -LiteralPath $item.FullName -Destination $game
}
if ($LeftScreen -or $MainScreen -or $RightScreen) {
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -Namespace Isolated -Name Dpi -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();'
    [Isolated.Dpi]::SetProcessDPIAware() | Out-Null
    $screen = if ($MainScreen) { [System.Windows.Forms.Screen]::PrimaryScreen } elseif ($RightScreen) { [System.Windows.Forms.Screen]::AllScreens | Sort-Object { $_.Bounds.X } | Select-Object -Last 1 } else { [System.Windows.Forms.Screen]::AllScreens | Sort-Object { $_.Bounds.X } | Select-Object -First 1 }
    $env:PALACLIPS_TEST_WINDOW = "$($screen.WorkingArea.X + 40),$($screen.WorkingArea.Y + 40)"
}
$env:PALACLIPS_CONFIG_DIR = $dir
$env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
$arguments = if ($Tray) { @{ ArgumentList = '--autostart' } } else { @{} }
Start-Process (Join-Path $root 'target\release\palaclips-app.exe') -PassThru @arguments
