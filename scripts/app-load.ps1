param(
    [int]$WarmupSeconds = 20,
    [int]$Seconds = 60,
    [string]$Name = "app-load"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$exe = Join-Path $root "target\release\palaclips-app.exe"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

Add-Type -AssemblyName System.Windows.Forms
Add-Type -Namespace Load -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
'@
[Load.Win]::SetProcessDPIAware() | Out-Null

function Get-Tree([int]$ProcessId) {
    $all = Get-CimInstance Win32_Process
    $ids = [System.Collections.Generic.List[int]]::new()
    $ids.Add($ProcessId)
    for ($i = 0; $i -lt $ids.Count; $i++) {
        $all | Where-Object { $_.ParentProcessId -eq $ids[$i] } | ForEach-Object { $ids.Add([int]$_.ProcessId) }
    }
    $ids
}

function Measure-Load([string]$Label, [int]$ProcessId) {
    $ids = Get-Tree $ProcessId
    $cores = [Environment]::ProcessorCount
    $start = @{}
    foreach ($id in $ids) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { $start[$id] = $p.TotalProcessorTime.TotalSeconds } }
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $paths = $ids | ForEach-Object { "\GPU Engine(pid_$($_)_*)\Utilization Percentage" }
    $samples = Get-Counter -Counter $paths -SampleInterval 1 -MaxSamples $Seconds -ErrorAction SilentlyContinue
    $elapsed = $clock.Elapsed.TotalSeconds
    $cpu = 0.0
    foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p -and $start.ContainsKey($id)) { $cpu += $p.TotalProcessorTime.TotalSeconds - $start[$id] }
    }
    $byEngine = @{}
    foreach ($sample in $samples) {
        $sums = @{}
        foreach ($value in $sample.CounterSamples) {
            if ($value.InstanceName -match 'engtype_(.+)$') { $sums[$Matches[1]] = ($sums[$Matches[1]] ?? 0) + $value.CookedValue }
        }
        foreach ($engine in $sums.Keys) { $byEngine[$engine] = @($byEngine[$engine]) + $sums[$engine] }
    }
    $gpu = ($byEngine.Keys | Sort-Object | ForEach-Object {
        $values = @($byEngine[$_] | Where-Object { $_ -ne $null })
        "{0} {1:N2} %" -f $_, (($values | Measure-Object -Sum).Sum / [Math]::Max(1, $samples.Count))
    }) -join ", "
    Write-Journal ("{0,-34} CPU {1:N2} % de la machine ({2:N1} % d'un cœur), {3} processus ; GPU moyen par moteur : {4}" -f $Label, (100 * $cpu / $elapsed / $cores), (100 * $cpu / $elapsed), $ids.Count, $gpu)
}

$app = $null
try {
    if (-not (Test-Path $exe)) { throw "compile d'abord : npx tauri build --no-bundle (dans app)" }
    if (Get-Process palaclips-app -ErrorAction SilentlyContinue) { throw "PalaClips tourne déjà : ferme-le avant la mesure" }
    $secondary = [System.Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary } | Select-Object -First 1
    if (-not $secondary) { throw "aucun écran secondaire" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $previous = [Load.Win]::GetForegroundWindow()
    Write-Journal "$(Get-Date -Format s) $Name : build release, replay armé (config par défaut), mise en régime $WarmupSeconds s, mesures de $Seconds s ; écran secondaire $($secondary.DeviceName) $($secondary.Bounds.Width)x$($secondary.Bounds.Height)"
    $app = Start-Process $exe -PassThru
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Process -Id $app.Id).MainWindowHandle -eq 0) {
        if ((Get-Date) -gt $deadline) { throw "fenêtre PalaClips introuvable" }
        Start-Sleep -Milliseconds 200
    }
    $window = (Get-Process -Id $app.Id).MainWindowHandle
    $area = $secondary.WorkingArea
    [Load.Win]::SetWindowPos($window, [IntPtr]::Zero, $area.X + 40, $area.Y + 40, 1196, 799, 0x0004 -bor 0x0010) | Out-Null
    [Load.Win]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
    [Load.Win]::SetForegroundWindow($previous) | Out-Null
    [Load.Win]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
    Start-Sleep -Seconds $WarmupSeconds
    $focused = [Load.Win]::GetForegroundWindow() -eq $window
    Write-Journal "fenêtre PalaClips au premier plan : $focused (attendu : False)"
    Measure-Load "fenêtre ouverte, sans focus" $app.Id
    [Load.Win]::PostMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Seconds 5
    Measure-Load "fenêtre fermée (zone de notification)" $app.Id
} catch {
    Write-Journal "ÉCHEC : $_"
} finally {
    if ($app -and -not $app.HasExited) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
    Get-Process msedgewebview2 -ErrorAction SilentlyContinue | Where-Object { $_.Parent -and $_.Parent.Id -eq $app.Id } | Stop-Process -Force -ErrorAction SilentlyContinue
    Write-Journal "$(Get-Date -Format s) fin : PalaClips arrêté, journal $journal"
}
