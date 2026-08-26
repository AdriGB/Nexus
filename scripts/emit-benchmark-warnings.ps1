[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ComparisonPath,
    [double]$CandidateGateThresholdPercent = 0
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-regression-report.ps1")
$observations = @(Get-BenchmarkPerformanceObservations -ComparisonPath $ComparisonPath)
Write-GitHubBenchmarkWarningAnnotations `
    -Observations $observations `
    -ExcludeCandidatesAbovePercent $CandidateGateThresholdPercent
