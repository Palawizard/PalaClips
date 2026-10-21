param([Parameter(Mandatory)][string]$Tag, [string]$Repository = "Palawizard/PalaClips")
$ErrorActionPreference = "Stop"

$previous = git describe --tags --abbrev=0 --match "v[0-9]*" "$Tag^" 2>$null
if ($LASTEXITCODE -ne 0) { $previous = $null }
$range = if ($previous) { "$previous..$Tag" } else { $Tag }

$groups = [ordered]@{ feat = "Features"; fix = "Bug fixes"; perf = "Performance"; docs = "Documentation"; other = "Maintenance" }
$internalScopes = @("handoff", "reports")
$entries = @{}
foreach ($key in $groups.Keys) { $entries[$key] = [System.Collections.Generic.List[string]]::new() }

foreach ($subject in git log --merges --reverse --format=%s $range) {
    $match = [regex]::Match($subject, '^(\w+)\(([^)]+)\)(!)?: (.+) \(#(\d+)\)$')
    if (-not $match.Success) { continue }
    $type, $scope, $breaking, $description, $number = $match.Groups[1..5].Value
    if ($scope -in $internalScopes) { continue }
    $key = if ($groups.Contains($type)) { $type } else { "other" }
    $line = "- **$scope**: $description$(if ($breaking) { ' (breaking)' }) (#$number)"
    if (-not $entries[$key].Contains($line)) { $entries[$key].Add($line) }
}

$notes = foreach ($key in $groups.Keys) {
    if ($entries[$key].Count -eq 0) { continue }
    "## $($groups[$key])"
    ""
    $entries[$key]
    ""
}
$notes += if ($previous) {
    "**Full changelog**: https://github.com/$Repository/compare/$previous...$Tag"
} else {
    "First public release. Details: [CHANGELOG.md](https://github.com/$Repository/blob/$Tag/CHANGELOG.md)."
}
$notes -join "`n"
