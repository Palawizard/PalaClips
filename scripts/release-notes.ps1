param([Parameter(Mandatory)][string]$Tag)
$ErrorActionPreference = "Stop"

$previous = git describe --tags --abbrev=0 --match "v[0-9]*" "$Tag^" 2>$null
if ($LASTEXITCODE -ne 0) { $previous = $null }
$range = if ($previous) { "$previous..$Tag" } else { $Tag }
$internalScopes = @("cli", "ci", "scripts", "docs", "handoff", "reports", "repo", "deps", "ipc")

foreach ($subject in git log --merges --reverse --format=%s $range) {
    $match = [regex]::Match($subject, '^(feat|fix|perf)\(([^)]+)\)!?: .+ \(#\d+\)$')
    if ($match.Success -and $match.Groups[2].Value -notin $internalScopes) { $subject }
}
