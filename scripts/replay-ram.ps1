param(
    [int]$Minutes = 120,
    [int]$Duration = 60,
    [int]$BitrateMbps = 40,
    [string]$ToneDevice = "Realtek",
    [string]$Name = "replay-ram"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$clips = Join-Path $root "target\clips\$Name"
New-Item -ItemType Directory -Force $journalDir | Out-Null
Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

Add-Type -AssemblyName System.Windows.Forms
Add-Type -Namespace PalaClips -Name Power -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);'
$continuous = [uint32]2147483648
[PalaClips.Power]::SetThreadExecutionState($continuous -bor 1 -bor 2) | Out-Null
$started = @()
try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $toneA = Join-Path $root "target\tone-a.exe"
    $toneB = Join-Path $root "target\tone-b.exe"
    Copy-Item $cli $toneA -Force
    Copy-Item $cli $toneB -Force
    $seconds = $Minutes * 60
    Write-Journal "$(Get-Date -Format s) $Name : replay $Duration s armé pendant $Minutes min, $BitrateMbps Mb/s 1440p60, 4 pistes (mix, tone-a, tone-b, Jeu), fenêtre de bruit ffplay"
    $area = [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
    $started += Start-Process (Join-Path $root "third_party\ffmpeg\bin\ffplay.exe") -ArgumentList @(
        "-hide_banner", "-loglevel", "error", "-an",
        "-x", "1280", "-y", "720", "-left", "$($area.Right - 1300)", "-top", "$($area.Bottom - 760)",
        "-f", "lavfi", "-i", "nullsrc=s=640x360:r=60,geq=random(1)*255:128:128"
    ) -PassThru
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Process -Id $started[0].Id).MainWindowHandle -eq 0) {
        if ((Get-Date) -gt $deadline) { throw "la fenêtre ffplay ne s'est pas ouverte" }
        Start-Sleep -Milliseconds 200
    }
    $started += Start-Process $toneA -ArgumentList "play-tone", "--hz", "440", "--seconds", "$($seconds + 120)", "--device", $ToneDevice -PassThru -NoNewWindow
    $started += Start-Process $toneB -ArgumentList "play-tone", "--hz", "880", "--seconds", "$($seconds + 120)", "--device", $ToneDevice -PassThru -NoNewWindow
    Start-Sleep -Seconds 2

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = "replay --target window:ffplay.exe --height 1440 --fps 60 --bitrate-mbps $BitrateMbps --duration $Duration --no-mic --app tone-a.exe --app tone-b.exe --clips-dir `"$clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $replay = [System.Diagnostics.Process]::Start($info)
    $started += $replay
    $stdout = $replay.StandardOutput.ReadToEndAsync()
    $stderr = $replay.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds 5
    if ($replay.HasExited) { throw "replay non armé : $($stdout.Result.Trim()) $($stderr.Result.Trim())" }
    Start-Sleep -Seconds $seconds
    $replay.StandardInput.WriteLine("")
    Start-Sleep -Seconds 5
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(60000)) { throw "le replay ne s'est pas arrêté" }
    $lines = $stdout.Result -split "`r?`n"
    $samples = foreach ($line in $lines) {
        if ($line -match '^\[\s*(\d+) s\].*mémoire privée du processus (\d+) Mo') {
            [pscustomobject]@{ Seconds = [int]$Matches[1]; Mb = [int]$Matches[2] }
        }
    }
    $lines | Where-Object { $_ -match 'replay de|état|clip|alerte|RÉSUMÉ' } | ForEach-Object { Write-Journal $_ }
    $samples | Where-Object { $_.Seconds % 600 -lt 5 } | ForEach-Object { Write-Journal ("{0,6} s : {1} Mo" -f $_.Seconds, $_.Mb) }
    Write-Journal ($stderr.Result -split "`n" | Select-String "WARN|ERROR" | Out-String).TrimEnd()
    $atHalfHour = $samples | Where-Object { $_.Seconds -ge 1800 } | Select-Object -First 1
    $atEnd = $samples | Select-Object -Last 1
    if (-not $atHalfHour -or -not $atEnd) { throw "pas assez de mesures de mémoire" }
    $change = ($atEnd.Mb - $atHalfHour.Mb) / $atHalfHour.Mb * 100
    $verdict = if ([math]::Abs($change) -lt 5) { "OK" } else { "ÉCHEC" }
    Write-Journal ("RÉSULTAT : mémoire privée {0} Mo à {1} s, {2} Mo à {3} s, écart {4:+0.0;-0.0} % → {5} (critère < 5 %)" -f $atHalfHour.Mb, $atHalfHour.Seconds, $atEnd.Mb, $atEnd.Seconds, $change, $verdict)
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    [PalaClips.Power]::SetThreadExecutionState($continuous) | Out-Null
    Write-Journal "$(Get-Date -Format s) fin : ffplay, sons de test et replay arrêtés, clip supprimé, veille rétablie, journal $journal"
}
