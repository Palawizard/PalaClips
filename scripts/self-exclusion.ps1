param(
    [string]$Device = "",
    [double]$Threshold = 0.6,
    [switch]$Keep,
    [string]$Name = "self-exclusion"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$journalDir = Join-Path $root "target\journals"
$out = Join-Path $root "target\test-$Name"
New-Item -ItemType Directory -Force $journalDir | Out-Null
Remove-Item $out -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$probe = $null
$external = $null

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

try {
    cargo build --release -q -p palaclips-cli
    if ($LASTEXITCODE -ne 0) { throw "compilation de palaclips-cli impossible" }
    $env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
    $cli = Join-Path $root "target\release\palaclips-cli.exe"
    $deviceArgument = if ($Device) { "--device `"$Device`"" } else { "" }
    Write-Journal "$(Get-Date -Format s) $Name : loopback de la sortie, « tout sauf ce processus » et « seulement l'autre processus » en même temps ; son de début d'enregistrement joué par la sonde à 2 et 7 s, son de sauvegarde joué chaque seconde par un autre processus"
    $external = Start-Process -FilePath $cli -ArgumentList "play-chime --count 14" -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $out "external.txt")
    $probe = Start-Process -FilePath $cli -ArgumentList "probe-exclusion --seconds 12 --chime-at 2 --chime-at 7 --include-pid $($external.Id) --out `"$out`" $deviceArgument" -NoNewWindow -PassThru -RedirectStandardOutput (Join-Path $out "probe.txt")
    if (-not $probe.WaitForExit(30000)) { throw "la sonde ne s'est pas arrêtée" }
    Get-Content (Join-Path $out "probe.txt") | ForEach-Object { Write-Journal $_ }
    $mka = Join-Path $out "captures.mka"
    ffmpeg -v error -y -f f32le -ar 48000 -ac 1 -i (Join-Path $out "endpoint.f32") -f f32le -ar 48000 -ac 1 -i (Join-Path $out "except-self.f32") -f f32le -ar 48000 -ac 1 -i (Join-Path $out "process.f32") -map 0 -map 1 -map 2 -c:a pcm_f32le -metadata:s:a:0 handler_name=Sortie -metadata:s:a:1 "handler_name=Tout sauf PalaClips" -metadata:s:a:2 "handler_name=Autre processus seul" $mka
    $tracks = python -I -X utf8 (Join-Path $root "scripts\find-chimes.py") $mka --threshold $Threshold --json | ConvertFrom-Json
    foreach ($track in $tracks) {
        $hits = ($track.chimes | ForEach-Object { "{0} {1:N3} s ({2:N2})" -f $(if ($_.sound -eq "sauvegarde") { "autre" } else { "sonde" }), $_.at_s, $_.score }) -join ", "
        Write-Journal ("{0} : {1}" -f $track.title, $(if ($hits) { $hits } else { "aucun son" }))
    }
    $own = @($tracks[1].chimes | Where-Object { $_.sound -ne "sauvegarde" }).Count
    Write-Journal ("son de la sonde dans « tout sauf PalaClips » : {0}" -f $(if ($own) { "PRÉSENT ($own)" } else { "absent" }))
    $reference = @($tracks[2].chimes | Where-Object { $_.sound -eq "sauvegarde" })
    foreach ($index in 0, 1) {
        $offsets = foreach ($hit in $tracks[$index].chimes | Where-Object { $_.sound -eq "sauvegarde" }) {
            $match = $reference | Sort-Object { [math]::Abs($_.at_s - $hit.at_s) } | Select-Object -First 1
            if ($match -and [math]::Abs($match.at_s - $hit.at_s) -lt 0.25) { 1000 * ($hit.at_s - $match.at_s) }
        }
        $sorted = @($offsets | Sort-Object)
        $median = if ($sorted.Count) { $sorted[[int][math]::Floor($sorted.Count / 2)] } else { $null }
        Write-Journal ("{0} : {1} sons de l'autre processus sur {2}, retard sur « autre processus seul » : médiane {3} ms ({4})" -f $tracks[$index].title, $sorted.Count, $reference.Count, $(if ($null -ne $median) { "{0:N1}" -f $median } else { "—" }), (($sorted | ForEach-Object { "{0:N0}" -f $_ }) -join " "))
    }
}
finally {
    foreach ($process in $probe, $external) { if ($process -and -not $process.HasExited) { $process.Kill($true) } }
    if (-not $Keep) { Get-ChildItem $out -Include *.f32, *.mka -Recurse -ErrorAction SilentlyContinue | Remove-Item -Force }
    Write-Journal "journal : $journal"
}
