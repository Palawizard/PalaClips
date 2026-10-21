param(
    [int]$Saves = 20,
    [int]$IntervalSeconds = 3,
    [int]$Duration = 60,
    [int]$BitrateMbps = 40,
    [string]$ToneDevice = "Realtek",
    [switch]$Disk,
    [string]$Name = "replay-save"
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
    $runSeconds = $Duration + 10 + $Saves * $IntervalSeconds + 30
    $storage = if ($Disk) { "disque" } else { "RAM" }
    Write-Journal "$(Get-Date -Format s) $Name : replay $Duration s ($storage), $BitrateMbps Mb/s 1440p60, 4 pistes (mix, tone-a, tone-b, Jeu), $Saves sauvegardes toutes les $IntervalSeconds s"
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
    $started += Start-Process $toneA -ArgumentList "play-tone", "--hz", "440", "--seconds", "$runSeconds", "--device", $ToneDevice -PassThru -NoNewWindow
    $started += Start-Process $toneB -ArgumentList "play-tone", "--hz", "880", "--seconds", "$runSeconds", "--device", $ToneDevice -PassThru -NoNewWindow
    Start-Sleep -Seconds 2

    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $cli
    $diskFlag = if ($Disk) { "--disk" } else { "" }
    $info.Arguments = "replay --target window:ffplay.exe --height 1440 --fps 60 --bitrate-mbps $BitrateMbps --duration $Duration --no-mic --app tone-a.exe --app tone-b.exe --clips-dir `"$clips`" $diskFlag"
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
    Start-Sleep -Seconds ($Duration + 5)
    for ($i = 0; $i -lt $Saves; $i++) {
        $replay.StandardInput.WriteLine("")
        Start-Sleep -Seconds $IntervalSeconds
    }
    $replay.StandardInput.WriteLine("q")
    if (-not $replay.WaitForExit(60000)) { throw "le replay ne s'est pas arrêté" }
    Write-Journal $stdout.Result.TrimEnd()
    Write-Journal ($stderr.Result -split "`n" | Select-String "replay armé|WARN|ERROR" | Out-String).TrimEnd()

    $failures = 0
    $files = Get-ChildItem $clips -Recurse -Filter *.mp4 | Sort-Object Name
    foreach ($file in $files) {
        $streams = ffprobe -v error -show_entries stream=codec_type,start_time,duration -of csv=p=0 $file.FullName
        $durations = $streams | ForEach-Object { [double]($_ -split ",")[2] }
        $starts = $streams | ForEach-Object { [double]($_ -split ",")[1] }
        $video = $durations[0]
        $spread = ($durations | Measure-Object -Maximum -Minimum | ForEach-Object { $_.Maximum - $_.Minimum })
        $ok = ($streams.Count -eq 5) -and ($video -ge $Duration - 1) -and ($video -le $Duration + 1) -and ($spread -le 0.05) -and (($starts | Measure-Object -Maximum).Maximum -eq 0)
        if (-not $ok) { $failures++ }
        Write-Journal ("{0} : {1:N0} Mo, {2} flux, vidéo {3:N3} s, écart entre flux {4:N3} s, débuts {5} → {6}" -f $file.Name, ($file.Length / 1MB), $streams.Count, $video, $spread, ($starts -join "/"), $(if ($ok) { "OK" } else { "ÉCHEC" }))
    }
    $failures += [math]::Max(0, $Saves - $files.Count)
    Write-Journal "RÉSULTAT : $($files.Count) clips sur $Saves, $failures hors critères (durée $Duration s +/- 1 intervalle d'images clés, pistes de même durée, 5 flux, début à 0)"
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    $started | Where-Object { $_ -and -not $_.HasExited } | Stop-Process -Force -ErrorAction SilentlyContinue
    Remove-Item $clips -Recurse -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : ffplay, sons de test et replay arrêtés, clips supprimés, journal $journal"
}
