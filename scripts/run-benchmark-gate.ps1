[CmdletBinding()]
param(
    [string]$ComparisonPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "target/nexus-bench/benchmark-comparison.json"),
    [string]$RetryOutputDir = (Join-Path (Split-Path -Parent $PSScriptRoot) "target/nexus-bench/retry"),
    [double]$CandidateThresholdPercent = 30.0,
    [AllowNull()][scriptblock]$RetryScript
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "benchmark-gate.ps1")

$result = Invoke-BenchmarkGate `
    -ComparisonPath $ComparisonPath `
    -RetryOutputDir $RetryOutputDir `
    -CandidateThresholdPercent $CandidateThresholdPercent `
    -RetryScript $RetryScript

Write-GitHubBenchmarkCandidateAnnouncements -Candidates @($result.Candidates)
Write-GitHubBenchmarkGateOutcomeAnnotations -Outcomes $result.Outcomes

$markdown = Get-BenchmarkGateMarkdown `
    -Outcomes $result.Outcomes `
    -CandidateThresholdPercent $result.CandidateThresholdPercent
$summaryPath = $env:GITHUB_STEP_SUMMARY
if ([string]::IsNullOrWhiteSpace($summaryPath)) {
    Write-Host $markdown
}
else {
    [System.IO.File]::AppendAllText($summaryPath, $markdown + [Environment]::NewLine)
}

if ($result.HasConfirmedRegression) {
    Write-Host "Confirmed performance regression(s): gate failed." -ForegroundColor Red
}
elseif ($result.HasTechnicalFailure) {
    Write-Host "Benchmark gate technical failure: confirmation run could not be classified." -ForegroundColor Red
}
else {
    Write-Host "Performance gate passed." -ForegroundColor Green
}

if ($result.HasConfirmedRegression -or $result.HasTechnicalFailure) {
    exit 1
}
exit 0
