param(
    [int]$Seconds = 30,
    [string]$Device = "Realtek",
    [string]$Name = "game-track"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$outDir = Join-Path $root "target\clips"
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $outDir, $journalDir | Out-Null
$stamp = Get-Date -Format "yyyy-MM-dd_HH-mm"
$journal = Join-Path $journalDir "$Name-$stamp.txt"
$video = Join-Path $outDir "$Name-$stamp.mp4"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

Add-Type -Namespace PalaClips -Name Focus -MemberDefinition @'
[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr window);
[DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, System.UIntPtr extra);
'@

function Set-Focus([System.IntPtr]$Window, [string]$Label) {
    [PalaClips.Focus]::keybd_event(0x12, 0, 0, [System.UIntPtr]::Zero)
    [PalaClips.Focus]::keybd_event(0x12, 0, 2, [System.UIntPtr]::Zero)
    [PalaClips.Focus]::SetForegroundWindow($Window) | Out-Null
    Start-Sleep -Milliseconds 300
    $ok = [PalaClips.Focus]::GetForegroundWindow() -eq $Window
    Write-Journal "$(Get-Date -Format HH:mm:ss.fff) premier plan → $Label : $(if ($ok) { 'OK' } else { 'REFUSÉ' })"
    if (-not $ok) { throw "Windows a refusé de passer $Label au premier plan" }
}

$previous = [PalaClips.Focus]::GetForegroundWindow()
$tones = @()
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $game = Join-Path $root "target\tone-game.exe"
    $chat = Join-Path $root "target\tone-chat.exe"
    Copy-Item $cli $game -Force
    Copy-Item $cli $chat -Force
    $toneSeconds = $Seconds + 10
    Write-Journal "$(Get-Date -Format s) $Name : « jeu » sans bordure plein écran 440 Hz (tone-game.exe), fenêtre normale 880 Hz (tone-chat.exe), sortie « $Device », capture $Seconds s"
    $tones += Start-Process $game -ArgumentList "play-tone", "--hz", "440", "--seconds", "$toneSeconds", "--device", $Device, "--window", "borderless" -PassThru -NoNewWindow
    $tones += Start-Process $chat -ArgumentList "play-tone", "--hz", "880", "--seconds", "$toneSeconds", "--device", $Device, "--window", "normal" -PassThru -NoNewWindow
    Start-Sleep -Seconds 2
    $gameWindow = (Get-Process -Id $tones[0].Id).MainWindowHandle
    $chatWindow = (Get-Process -Id $tones[1].Id).MainWindowHandle
    if ($gameWindow -eq 0 -or $chatWindow -eq 0) { throw "fenêtres de test introuvables" }
    Set-Focus $gameWindow "jeu sans bordure"
    $third = [int]($Seconds / 3)
    $capture = Start-Job -ScriptBlock {
        param($root, $cli, $arguments)
        [Console]::OutputEncoding = [Text.Encoding]::UTF8
        Set-Location $root
        $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
        & $cli @arguments 2>&1 | Out-String -Width 300
    } -ArgumentList $root, $cli, @(
        "capture-apps", "--target", "primary", "--seconds", "$Seconds", "--out", $video,
        "--no-mix", "--game", "--app", "tone-chat.exe", "--fps", "30", "--height", "360", "--bitrate-mbps", "1"
    )
    Start-Sleep -Seconds $third
    Set-Focus $chatWindow "fenêtre normale (Alt+Tab)"
    Start-Sleep -Seconds $third
    Set-Focus $gameWindow "retour au jeu"
    Write-Journal (Receive-Job $capture -Wait -AutoRemoveJob).TrimEnd()
    if (-not (Test-Path $video)) { throw "aucune vidéo produite" }
    Write-Journal "secondes de capture : jeu au premier plan de 0 à $third s, fenêtre normale de $third à $(2 * $third) s, jeu ensuite"
    Write-Journal (python -I -X utf8 (Join-Path $root "scripts\measure-tones.py") $video --hz 440 --hz 880 | Out-String).TrimEnd()
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $tones | Where-Object { -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $video -ErrorAction SilentlyContinue
    if ($previous -ne 0) { [PalaClips.Focus]::SetForegroundWindow($previous) | Out-Null }
    Write-Journal "$(Get-Date -Format s) fin, fenêtres de test fermées, vidéo supprimée, premier plan rendu, journal $journal"
}
