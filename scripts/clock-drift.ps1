param(
    [int]$Minutes = 20,
    [string[]]$Devices = @("Apogee", "Voicemeeter Input"),
    [string]$Name = "clock-drift"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

Add-Type -Namespace PalaClips -Name Power -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);'
$continuous = [uint32]2147483648
[PalaClips.Power]::SetThreadExecutionState($continuous -bor 1) | Out-Null
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $seconds = $Minutes * 60
    foreach ($device in $Devices) {
        Write-Journal "$(Get-Date -Format s) $Name : $Minutes min, silence numérique joué sur « $device », loopback de la même sortie"
        $tone = Start-Process $cli -ArgumentList "play-tone", "--amplitude", "0", "--seconds", "$($seconds + 30)", "--device", "`"$device`"" -NoNewWindow -PassThru
        Start-Sleep -Seconds 3
        & $cli probe-audio --device $device --seconds $seconds 2>&1 | Out-String -Width 400 | ForEach-Object { Write-Journal $_.TrimEnd() }
        Stop-Process -Id $tone.Id -ErrorAction SilentlyContinue
    }
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    [PalaClips.Power]::SetThreadExecutionState($continuous) | Out-Null
    Write-Journal "$(Get-Date -Format s) fin, journal $journal"
}
