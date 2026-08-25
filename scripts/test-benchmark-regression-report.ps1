Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-regression-report.ps1")

function Assert-Equal($Expected, $Actual, [string]$Message) {
    if ($Expected -cne $Actual) { throw "$Message Expected '$Expected', got '$Actual'." }
}
function Assert-Throws([scriptblock]$Action, [string]$Message) {
    try { & $Action | Out-Null } catch { return }
    throw $Message
}
function New-Scenario(
    [string]$Name,
    $MeanDelta,
    [double]$BaselineMean = 1000,
    [double]$CurrentMean = 1100,
    [double]$P95Delta = 4,
    [double]$PhaseDelta = 0
) {
    [ordered]@{
        name = $Name
        timings = [ordered]@{
            total = [ordered]@{
                mean = [ordered]@{ baseline_us = $BaselineMean; current_us = $CurrentMean; delta_percent = $MeanDelta }
                p95 = [ordered]@{ baseline_us = 1200; current_us = 1248; delta_percent = $P95Delta }
            }
            households = [ordered]@{ mean = [ordered]@{ delta_percent = $PhaseDelta } }
        }
    }
}
function Write-Comparison([string]$Path, [array]$Scenarios, [int]$Schema = 1) {
    $payload = [ordered]@{ schema_version = $Schema; scenarios = $Scenarios }
    [IO.File]::WriteAllText($Path, ($payload | ConvertTo-Json -Depth 20), [Text.UTF8Encoding]::new($false))
}

$root = Join-Path ([IO.Path]::GetTempPath()) "nexus-report-$([guid]::NewGuid())"
New-Item -ItemType Directory $root | Out-Null
try {
    $path = Join-Path $root "comparison.json"
    Write-Comparison $path @(
        (New-Scenario "below" 9.99),
        (New-Scenario "boundary" 10.0),
        (New-Scenario "just-over" 10.01 1000 1100.1 7),
        (New-Scenario "large" 25),
        (New-Scenario "faster" -20),
        (New-Scenario "unavailable" $null),
        (New-Scenario "phase-noise" 5 1000 1050 3 500),
        (New-Scenario "total-only" 11 1000 1110 2 1)
    )
    $items = @(Get-ReportableBenchmarkSlowdowns $path)
    Assert-Equal 3 $items.Count "Reportable count differs."
    Assert-Equal "large" $items[0].Name "Descending order differs."
    Assert-Equal "total-only" $items[1].Name "Total-only signal was not reported."
    Assert-Equal "just-over" $items[2].Name "Strict threshold differs."
    Assert-Equal 1000 $items[2].BaselineMeanUs "Baseline mean was not preserved."
    Assert-Equal 1100.1 $items[2].CurrentMeanUs "Current mean was not preserved."
    Assert-Equal 7 $items[2].P95DeltaPercent "p95 context was not preserved."

    Write-Comparison $path @((New-Scenario "zeta" 15), (New-Scenario "alpha" 15))
    $ties = @(Get-ReportableBenchmarkSlowdowns $path)
    Assert-Equal "alpha" $ties[0].Name "Ordinal tie order differs."
    Assert-Equal "zeta" $ties[1].Name "Ordinal tie order differs."

    Write-Comparison $path @((New-Scenario "none" 10))
    $none = @(Get-ReportableBenchmarkSlowdowns $path)
    Assert-Equal 0 $none.Count "Zero-report case differs."
    $markdown = Get-InformationalBenchmarkMarkdown $none
    if ($markdown -notmatch "None\.") { throw "Zero-report Markdown is unclear." }

    Write-Comparison $path @((New-Scenario "bad-schema" 11)) 2
    Assert-Throws { Get-ReportableBenchmarkSlowdowns $path } "Invalid comparison schema was accepted."

    $malformed = New-Scenario "malformed" 11
    $malformed.timings.total.p95 = $null
    Write-Comparison $path @($malformed)
    Assert-Throws { Get-ReportableBenchmarkSlowdowns $path } "Malformed timing was accepted."

    Write-Host "Benchmark regression report tests passed." -ForegroundColor Green
}
finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
