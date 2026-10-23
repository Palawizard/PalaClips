param(
    [int]$AfterSeconds = 600,
    [int]$Before = 60,
    [string]$Name = "post-roll-memory"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$isolated = Join-Path $root "target\test-$Name"
$clips = Join-Path $isolated "clips"
New-Item -ItemType Directory -Force $journalDir | Out-Null
Remove-Item $isolated -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $clips | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

function Get-PrivateMb([int]$ProcessId) {
    $sample = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process -Filter "IDProcess = $ProcessId" -ErrorAction SilentlyContinue
    if ($sample) { [math]::Round($sample.WorkingSetPrivate / 1MB, 1) } else { $null }
}

Add-Type -Namespace PalaClips -Name Power -MemberDefinition '[DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);'
$continuous = [uint32]2147483648
[PalaClips.Power]::SetThreadExecutionState($continuous -bor 1 -bor 2) | Out-Null
$replay = $null
try {
    Write-Journal "$(Get-Date -Format s) $Name : avant $Before s, après $AfterSeconds s, écran principal 360p30 2 Mb/s, piste mix ; mémoire privée (working set privé) toutes les 10 s"
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $info.Arguments = "replay --target primary --height 360 --fps 30 --bitrate-mbps 2 --duration $Before --after $AfterSeconds --no-mic --no-game --clips-dir `"$clips`""
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.UseShellExecute = $false
    $info.StandardOutputEncoding = [Text.Encoding]::UTF8
    $info.EnvironmentVariables["PALACLIPS_CONFIG_DIR"] = $isolated
    $replay = [System.Diagnostics.Process]::Start($info)
    $stdout = $replay.StandardOutput.ReadToEndAsync()
    $stderr = $replay.StandardError.ReadToEndAsync()
    Start-Sleep -Seconds ($Before + 10)
    Write-Journal "$(Get-Date -Format s) tampon plein : $(Get-PrivateMb $replay.Id) Mo ; appui"
    $replay.StandardInput.WriteLine("")
    $samples = @()
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt $AfterSeconds) {
        Start-Sleep -Seconds 10
        $mb = Get-PrivateMb $replay.Id
        if ($null -eq $mb) { throw "le replay s'est arrêté pendant le clip en cours" }
        $samples += [pscustomobject]@{ T = [math]::Round($clock.Elapsed.TotalSeconds); Mb = $mb }
        if ($samples.Count % 6 -eq 0) { Write-Journal ("{0,5} s : {1} Mo" -f $samples[-1].T, $mb) }
    }
    Start-Sleep -Seconds 15
    Write-Journal "$(Get-Date -Format s) après la fin du clip : $(Get-PrivateMb $replay.Id) Mo"
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(60000)) { throw "le replay ne s'est pas arrêté" }
    Write-Journal (($stdout.Result -split "`n" | Select-String "clip|alerte") -join "`n")
    Write-Journal (($stderr.Result -split "`n" | Select-String "WARN|ERROR") -join "`n")

    $steady = $samples | Where-Object { $_.T -ge 60 }
    $n = $steady.Count
    $meanT = ($steady | Measure-Object T -Average).Average
    $meanM = ($steady | Measure-Object Mb -Average).Average
    $slope = ($steady | ForEach-Object { ($_.T - $meanT) * ($_.Mb - $meanM) } | Measure-Object -Sum).Sum /
        ($steady | ForEach-Object { [math]::Pow($_.T - $meanT, 2) } | Measure-Object -Sum).Sum
    $range = $steady | Measure-Object Mb -Minimum -Maximum
    Write-Journal ("mémoire pendant le clip (de 60 s à la fin, {0} mesures) : min {1} Mo, max {2} Mo, pente {3:+0.0;-0.0} Mo/h" -f $n, $range.Minimum, $range.Maximum, ($slope * 3600))
    $file = Get-ChildItem $clips -Recurse -Filter *_replay.mp4 | Select-Object -First 1
    if (-not $file) { throw "aucun clip" }
    $check = python -I -X utf8 (Join-Path $root "scripts\clip-continuity.py") $file.FullName --json | ConvertFrom-Json
    $detail = ($check.streams | ForEach-Object { "$($_.type) {0:N1} s, doublons $($_.duplicates), trous $($_.gaps)" -f $_.duration_s }) -join " ; "
    Write-Journal "clip : $([math]::Round($file.Length / 1MB)) Mo ; $detail"
    $ok = $check.continuous -and [math]::Abs($slope * 3600) -lt 20 -and ($range.Maximum - $range.Minimum) -lt 30
    Write-Journal "RÉSULTAT : $(if ($ok) { 'OK' } else { 'ÉCHEC' }) (critère : pente < 20 Mo/h, écart < 30 Mo, clip continu)"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    if ($replay -and -not $replay.HasExited) { $replay.Kill() }
    [PalaClips.Power]::SetThreadExecutionState($continuous) | Out-Null
    Remove-Item $isolated -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : replay arrêté, mise en veille rétablie, dossier isolé supprimé, journal $journal"
}
