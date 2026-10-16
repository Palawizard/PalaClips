param(
    [int]$Minutes = 20,
    [int]$SaveEverySeconds = 5,
    [int]$Duration = 60,
    [int]$BitrateMbps = 40,
    [double]$MaxMbPerSave = 0.1,
    [double]$MaxMbPerHour = 20,
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

function Get-Slope($Points) {
    $n = $Points.Count
    $mx = ($Points | Measure-Object -Property X -Average).Average
    $my = ($Points | Measure-Object -Property Y -Average).Average
    $sxx = 0.0; $sxy = 0.0
    foreach ($p in $Points) { $sxx += ($p.X - $mx) * ($p.X - $mx); $sxy += ($p.X - $mx) * ($p.Y - $my) }
    $slope = $sxy / $sxx
    $residual = 0.0
    foreach ($p in $Points) { $r = $p.Y - $my - $slope * ($p.X - $mx); $residual += $r * $r }
    [pscustomobject]@{ Slope = $slope; StdErr = [math]::Sqrt($residual / ($n - 2) / $sxx) }
}

function Remove-FinishedClips {
    Get-ChildItem $clips -Recurse -Filter *.mp4 -ErrorAction SilentlyContinue |
        Where-Object { $_.LastWriteTime -lt (Get-Date).AddSeconds(-3) } |
        Remove-Item -Force -ErrorAction SilentlyContinue
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
    $saves = [int]($seconds / $SaveEverySeconds)
    Write-Journal "$(Get-Date -Format s) $Name : replay $Duration s armé $Minutes min, une sauvegarde toutes les $SaveEverySeconds s ($saves), $BitrateMbps Mb/s 1440p60, 4 pistes (mix, tone-a, tone-b, Jeu), fenêtre de bruit ffplay ; clips effacés au fil de l'eau"
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
    for ($i = 0; $i -lt $saves; $i++) {
        $replay.StandardInput.WriteLine("")
        Start-Sleep -Seconds $SaveEverySeconds
        Remove-FinishedClips
    }
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(60000)) { throw "le replay ne s'est pas arrêté" }
    Remove-FinishedClips

    $lines = $stdout.Result -split "`r?`n"
    $samples = foreach ($line in $lines) {
        if ($line -match '^\[\s*(\d+) s\] clip (\d+) :.*mémoire privée ([\d.]+) Mo') {
            [pscustomobject]@{ Seconds = [int]$Matches[1]; Save = [int]$Matches[2]; Mb = [double]$Matches[3] }
        }
    }
    $lines | Where-Object { $_ -match 'replay de|état|alerte|RÉSUMÉ' } | ForEach-Object { Write-Journal $_ }
    $samples | Where-Object { $_.Save % 12 -eq 0 } | ForEach-Object { Write-Journal ("{0,6} s, sauvegarde {1,4} : {2,7:N1} Mo" -f $_.Seconds, $_.Save, $_.Mb) }
    Write-Journal ($stderr.Result -split "`n" | Select-String "WARN|ERROR" | Out-String).TrimEnd()
    $steady = @($samples | Where-Object { $_.Seconds -ge $Duration + 60 })
    if ($steady.Count -lt 10) { throw "pas assez de sauvegardes après la mise en régime ($($steady.Count))" }
    $perSave = Get-Slope ($steady | ForEach-Object { [pscustomobject]@{ X = $_.Save; Y = $_.Mb } })
    $perHour = Get-Slope ($steady | ForEach-Object { [pscustomobject]@{ X = $_.Seconds / 3600; Y = $_.Mb } })
    $missing = $saves - $samples.Count
    $ok = ($perSave.Slope -lt $MaxMbPerSave) -and ($perHour.Slope -lt $MaxMbPerHour) -and ($missing -eq 0)
    Write-Journal ("RÉSULTAT : {0} clips sur {1}, {2} mesures après {3} s ; pente {4:+0.000;-0.000} ± {5:0.000} Mo par sauvegarde (seuil {6}), {7:+0.0;-0.0} ± {8:0.0} Mo/h (seuil {9}) → {10}" -f $samples.Count, $saves, $steady.Count, ($Duration + 60), $perSave.Slope, $perSave.StdErr, $MaxMbPerSave, $perHour.Slope, $perHour.StdErr, $MaxMbPerHour, $(if ($ok) { "OK" } else { "ÉCHEC" }))
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    [PalaClips.Power]::SetThreadExecutionState($continuous) | Out-Null
    Write-Journal "$(Get-Date -Format s) fin : ffplay, sons de test et replay arrêtés, clips supprimés, veille rétablie, journal $journal"
}
