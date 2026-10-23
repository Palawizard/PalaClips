param(
    [Parameter(Mandatory)][string]$Dir,
    [string]$Name = "compress-check"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$journalDir = Join-Path $root "target\journals"
New-Item -ItemType Directory -Force $journalDir | Out-Null
$journal = Join-Path $journalDir "$Name-$(Get-Date -Format "yyyy-MM-dd_HH-mm").txt"
$out = Join-Path $Dir "out"
New-Item -ItemType Directory -Force $out | Out-Null
$env:PATH = "$root\third_party\ffmpeg\bin;$env:PATH"
$env:PALACLIPS_CONFIG_DIR = Join-Path $Dir "cfg"
$cli = Join-Path $root "target\release\palaclips-cli.exe"

function Write-Journal([string]$Line) {
    $Line | Tee-Object -FilePath $journal -Append | Write-Host
}

$cases = @(
    @{ File = "short-1440p60.mp4"; Mb = 20; Extra = @() },
    @{ File = "short-1440p60.mp4"; Mb = 5; Extra = @() },
    @{ File = "long-1440p60.mp4"; Mb = 20; Extra = @() },
    @{ File = "long-1440p60.mp4"; Mb = 50; Extra = @() },
    @{ File = "long-1440p60.mp4"; Mb = 500; Extra = @() },
    @{ File = "long-1440p60.mp4"; Mb = 20; Extra = @("--codec", "av1") },
    @{ File = "long-multi.mp4"; Mb = 50; Extra = @("--all-tracks") },
    @{ File = "long-1440p60.mp4"; Mb = 10; Extra = @("--start", "30", "--end", "60") },
    @{ File = "hdr10.mp4"; Mb = 10; Extra = @() },
    @{ File = "external.mkv"; Mb = 3; Extra = @() },
    @{ File = "external.mov"; Mb = 3; Extra = @() }
)
$failures = 0
Write-Journal "$(Get-Date -Format s) $Name : $($cases.Count) compressions, sorties dans $out"
foreach ($case in $cases) {
    $input = Join-Path $Dir $case.File
    $label = "$([IO.Path]::GetFileNameWithoutExtension($case.File))-$($case.Mb)MB$(($case.Extra -join '') -replace '[^a-z0-9]', '')"
    $target = Join-Path $out "$label.mp4"
    $lines = & $cli compress $input --target-mb $case.Mb --out $target @($case.Extra) 2>&1 | ForEach-Object { "$_" } | Where-Object { $_ -match "^(source|plan|RÉSULTAT)|error|Error" }
    $lines | ForEach-Object { Write-Journal "  $_" }
    if (-not (Test-Path $target)) {
        $failures++
        Write-Journal "$label : ÉCHEC, aucun fichier"
        continue
    }
    $bytes = (Get-Item $target).Length
    $limit = [int64]$case.Mb * 1000000
    $probe = ffprobe -v error -show_entries stream=codec_type,codec_name,width,height,avg_frame_rate,color_transfer:stream_tags=title:format=duration -of compact $target
    $ok = $bytes -le $limit
    if (-not $ok) { $failures++ }
    Write-Journal ("{0} : {1:N0} octets pour {2:N0} ({3:P1} de la cible) → {4}" -f $label, $bytes, $limit, ($bytes / $limit), $(if ($ok) { "OK" } else { "ÉCHEC" }))
    $probe | ForEach-Object { Write-Journal "    $_" }
}
Write-Journal "RÉSULTAT : $failures compression(s) hors critère (taille ≤ cible)"
