param(
    [int]$Resizes = 10,
    [int]$SettleSeconds = 4,
    [string]$Name = "resize-leak"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$clips = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

Add-Type -Namespace PalaClips -Name Window -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetWindowPos(System.IntPtr window, System.IntPtr after, int x, int y, int width, int height, uint flags);
[DllImport("user32.dll")] public static extern bool GetWindowRect(System.IntPtr window, int[] rect);
'@

function Get-WindowSize([System.IntPtr]$Window) {
    $rect = New-Object int[] 4
    [PalaClips.Window]::GetWindowRect($Window, $rect) | Out-Null
    "$($rect[2] - $rect[0])×$($rect[3] - $rect[1])"
}

function Get-PrivateWorkingSet([int]$ProcessId) {
    $sample = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess = $ProcessId"
    [math]::Round($sample.WorkingSetPrivate / 1MB, 1)
}

$started = @()
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $started += Start-Process (Join-Path $root "third_party\ffmpeg\bin\ffplay.exe") -ArgumentList @(
        "-hide_banner", "-loglevel", "error", "-an",
        "-x", "640", "-y", "360", "-left", "200", "-top", "200",
        "-f", "lavfi", "-i", "testsrc2=s=640x360:r=30"
    ) -PassThru
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Process -Id $started[0].Id).MainWindowHandle -eq 0) {
        if ((Get-Date) -gt $deadline) { throw "fenêtre de test introuvable" }
        Start-Sleep -Milliseconds 200
    }
    $window = (Get-Process -Id $started[0].Id).MainWindowHandle

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = "replay --target window:ffplay.exe --height 360 --fps 30 --bitrate-mbps 1 --duration 30 --no-mic --no-mix --no-game --clips-dir `"$clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardErrorEncoding = [Text.Encoding]::UTF8
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $replay =[System.Diagnostics.Process]::Start($info)
    $started += $replay
    $stdout = $replay.StandardOutput.ReadToEndAsync()
    $stderr = $replay.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 8
    if ($replay.HasExited) { throw "replay non armé : $($stderr.Result.Trim())" }
    Write-Journal "$(Get-Date -Format s) $Name : replay sur une fenêtre ffplay animée (mire testsrc2), $Resizes redimensionnements (1280×720 ↔ 800×450), mémoire relevée $SettleSeconds s après chacun"
    $baseline = Get-PrivateWorkingSet $replay.Id
    Write-Journal ("départ : {0} Mo" -f $baseline)
    $samples = @()
    for ($i = 1; $i -le $Resizes; $i++) {
        $size = if ($i % 2) { @(1280, 720) } else { @(800, 450) }
        [PalaClips.Window]::SetWindowPos($window, [System.IntPtr]::Zero, 200, 200, $size[0], $size[1], 0x0014) | Out-Null
        Start-Sleep -Seconds $SettleSeconds
        $memory = Get-PrivateWorkingSet $replay.Id
        $samples += $memory
        Write-Journal ("redimensionnement {0,2} → {1}×{2} (fenêtre {3}) : {4} Mo" -f $i, $size[0], $size[1], (Get-WindowSize $window), $memory)
    }
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(30000)) { throw "le replay ne s'est pas arrêté" }
    $engineLines = ($stdout.Result + "`n" + $stderr.Result) -split "`n" | Select-String "résolution|WARN|ERROR|arrêtée" | ForEach-Object { $_.Line -replace "`e\[[0-9;]*m", '' }
    $changes = ($engineLines | Select-String "changement de résolution").Count
    Write-Journal ($engineLines -join "`n")
    Write-Journal "changements de résolution vus par le moteur : $changes"

    $tail = $samples | Select-Object -Skip 2
    $n = $tail.Count
    $xs = 1..$n
    $meanX = ($xs | Measure-Object -Average).Average
    $meanY = ($tail | Measure-Object -Average).Average
    $num = 0; $den = 0
    for ($k = 0; $k -lt $n; $k++) { $num += ($xs[$k] - $meanX) * ($tail[$k] - $meanY); $den += [math]::Pow($xs[$k] - $meanX, 2) }
    $slope = [math]::Round($num / $den, 2)
    $leak = $slope -gt 1.0
    Write-Journal "RÉSULTAT : pente $slope Mo par redimensionnement (après les 2 premiers) → $(if ($leak) { 'FUITE' } else { 'stable' })"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : fenêtre de test et replay arrêtés, journal $journal"
}
