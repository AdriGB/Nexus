Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-regression-report.ps1")

function Assert-Equal($Expected, $Actual, [string]$Message) { if ($Expected -cne $Actual) { throw "$Message Expected '$Expected', got '$Actual'." } }
function Assert-Throws([scriptblock]$Action, [string]$Message) { try { & $Action | Out-Null } catch { return }; throw $Message }
function New-Scenario([string]$Name, $MeanDelta, [double]$BaselineMean = 1000, [double]$CurrentMean = 1100, [double]$P95Delta = 4, [double]$PhaseDelta = 0) {
    [ordered]@{ name = $Name; timings = [ordered]@{
        total = [ordered]@{ mean = [ordered]@{ baseline_us = $BaselineMean; current_us = $CurrentMean; delta_percent = $MeanDelta }; p95 = [ordered]@{ baseline_us = 1200; current_us = 1248; delta_percent = $P95Delta } }
        households = [ordered]@{ mean = [ordered]@{ delta_percent = $PhaseDelta } }
    }}
}
function Write-Comparison([string]$Path, [array]$Scenarios, [int]$Schema = 1) { [IO.File]::WriteAllText($Path, ([ordered]@{ schema_version = $Schema; scenarios = $Scenarios } | ConvertTo-Json -Depth 20), [Text.UTF8Encoding]::new($false)) }

$root = Join-Path ([IO.Path]::GetTempPath()) "nexus-report-$([guid]::NewGuid())"
New-Item -ItemType Directory $root | Out-Null
try {
    $path = Join-Path $root "comparison.json"
    Write-Comparison $path @(
        (New-Scenario "below" 9.99), (New-Scenario "ten" 10.0), (New-Scenario "info-low" 10.01), (New-Scenario "info-high" 20.0),
        (New-Scenario "warning-low" 20.01 1000 1200.1 7), (New-Scenario "warning-large" 100), (New-Scenario "negative" -20),
        (New-Scenario "unavailable" $null), (New-Scenario "phase-noise" 5 1000 1050 3 500), (New-Scenario "total-info" 15 1000 1150 2 1),
        (New-Scenario "total-warning" 25 1000 1250 2 1), (New-Scenario "warning-thirty" 30)
    )
    $items = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal 7 $items.Count "Observation count differs."
    Assert-Equal "warning-large" $items[0].Name "Warnings must sort first by delta."
    Assert-Equal "warning-thirty" $items[1].Name "Warnings must sort before info."
    Assert-Equal "total-warning" $items[2].Name "Warning ordering differs."
    Assert-Equal "warning-low" $items[3].Name "Warning boundary differs."
    Assert-Equal "Warning" $items[3].Level "20.01 must be warning."
    Assert-Equal "Informational" $items[4].Level "20.00 must remain informational."
    Assert-Equal "total-info" $items[5].Name "Informational ordering differs."
    Assert-Equal 1000 $items[3].BaselineMeanUs "Baseline mean was not preserved."
    Assert-Equal 1200.1 $items[3].CurrentMeanUs "Current mean was not preserved."
    Assert-Equal 7 $items[3].P95DeltaPercent "p95 context was not preserved."

    Write-Comparison $path @((New-Scenario "zeta" 25), (New-Scenario "alpha" 25), (New-Scenario "info-z" 15), (New-Scenario "info-a" 15))
    $ties = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal "alpha" $ties[0].Name "Warning tie order differs."
    Assert-Equal "zeta" $ties[1].Name "Warning tie order differs."
    Assert-Equal "info-a" $ties[2].Name "Info tie order differs."
    $markdown = Get-InformationalBenchmarkMarkdown $ties
    if ($markdown.IndexOf("| Warning | alpha") -gt $markdown.IndexOf("| Info | info-a")) { throw "Markdown does not put warnings first." }
    $annotations = @(Write-GitHubBenchmarkWarningAnnotations $ties)
    Assert-Equal 2 $annotations.Count "Warnings must emit one annotation each."
    if ($annotations[0] -notmatch '^::warning title=Potential performance slowdown::alpha%3A total mean \+25\.00%') { throw "Warning annotation is malformed." }
    if ($annotations -match "info-a") { throw "Informational scenarios emitted warnings." }

    Write-Comparison $path @((New-Scenario "none" 10))
    $none = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal 0 $none.Count "Zero-report case differs."
    if ((Get-InformationalBenchmarkMarkdown $none) -notmatch "No performance slowdowns") { throw "Zero-report Markdown is unclear." }

    Write-Comparison $path @((New-Scenario "bad-schema" 11)) 2
    Assert-Throws { Get-BenchmarkPerformanceObservations $path } "Invalid comparison schema was accepted."
    $malformed = New-Scenario "malformed" 11; $malformed.timings.total.p95 = $null
    Write-Comparison $path @($malformed)
    Assert-Throws { Get-BenchmarkPerformanceObservations $path } "Malformed timing was accepted."

    Write-Host "Benchmark regression report tests passed." -ForegroundColor Green
}
finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase)) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
