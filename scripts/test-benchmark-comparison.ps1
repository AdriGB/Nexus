Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-comparison.ps1")

function Assert-Equal($Expected, $Actual, [string]$Message) {
    if ($Expected -cne $Actual) { throw "$Message Expected '$Expected', got '$Actual'." }
}
function Assert-Throws([scriptblock]$Action, [string]$Message) {
    try { & $Action | Out-Null } catch { return }
    throw $Message
}
function New-Stats([double]$Value) {
    [ordered]@{ mean_us = $Value; median_us = $Value; p95_us = $Value; p99_us = $Value; max_us = $Value }
}
function New-Summary([double]$Value) {
    $summary = [ordered]@{ samples = 10 }
    foreach ($category in $script:TimingCategories) { $summary[$category] = New-Stats $Value }
    return $summary
}
function New-Result([string]$Name, [double]$Value, [bool]$LongRun = $false) {
    $result = [ordered]@{
        schema_version = 3
        scenario = [ordered]@{
            name = $Name; seed = 42; population = 100; warmup_ticks = 2; measured_ticks = 10
            world = [ordered]@{ width = 64; height = 64; sea_level = 0.35 }
            workload = [ordered]@{ kind = "baseline" }
        }
    }
    if ($LongRun) {
        $result.overall = New-Summary $Value
        $result.windows = @([ordered]@{ index = 0; summary = (New-Summary $Value) })
    } else { $result.summary = New-Summary $Value }
    return $result
}
function Write-Aggregate([string]$Path, [string]$Suite, [array]$Results, [int]$Schema = 1) {
    $payload = [ordered]@{ schema_version = $Schema; suite = $Suite; results = $Results }
    [IO.File]::WriteAllText($Path, ($payload | ConvertTo-Json -Depth 100), [Text.UTF8Encoding]::new($false))
}

$root = Join-Path ([IO.Path]::GetTempPath()) "nexus-comparison-$([guid]::NewGuid())"
New-Item -ItemType Directory $root | Out-Null
try {
    $baselinePath = Join-Path $root "baseline.json"
    $currentPath = Join-Path $root "current.json"
    $outputPath = Join-Path $root "comparison.json"
    Write-Aggregate $baselinePath "full" @((New-Result "beta" 100), (New-Result "alpha" 100), (New-Result "extra" 100))
    Write-Aggregate $currentPath "quick" @((New-Result "beta" 90), (New-Result "alpha" 110))
    Write-BenchmarkComparison $baselinePath $currentPath $outputPath | Out-Null
    $comparison = Get-Content $outputPath -Raw | ConvertFrom-Json
    Assert-Equal 1 $comparison.schema_version "Comparison schema differs."
    Assert-Equal 2 $comparison.summary.compared_scenarios "Subset count differs."
    Assert-Equal "alpha" $comparison.scenarios[0].name "Scenario order differs."
    Assert-Equal "beta" $comparison.scenarios[1].name "Scenario order differs."
    Assert-Equal 10 $comparison.scenarios[0].timings.total.mean.delta_percent "Positive delta differs."
    Assert-Equal -10 $comparison.scenarios[1].timings.total.mean.delta_percent "Negative delta differs."
    foreach ($category in $script:TimingCategories) {
        foreach ($stat in $script:TimingStatistics.Keys) {
            Assert-Equal 100 $comparison.scenarios[0].timings.$category.$stat.baseline_us "Baseline timing differs."
            Assert-Equal 110 $comparison.scenarios[0].timings.$category.$stat.current_us "Current timing differs."
        }
    }
    $bytes = [IO.File]::ReadAllBytes($outputPath)
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) { throw "Comparison has a BOM." }

    Write-Aggregate $currentPath "scenario:alpha" @((New-Result "alpha" 100))
    Write-BenchmarkComparison $baselinePath $currentPath $outputPath | Out-Null
    $single = Get-Content $outputPath -Raw | ConvertFrom-Json
    Assert-Equal 1 $single.scenarios.Count "Single scenario count differs."
    Assert-Equal 0 $single.scenarios[0].timings.total.mean.delta_percent "Zero delta differs."

    Write-Aggregate $baselinePath "full" @((New-Result "zero" 0), (New-Result "zero-both" 0))
    Write-Aggregate $currentPath "quick" @((New-Result "zero" 1), (New-Result "zero-both" 0))
    Write-BenchmarkComparison $baselinePath $currentPath $outputPath | Out-Null
    $zero = Get-Content $outputPath -Raw | ConvertFrom-Json
    if ($null -ne $zero.scenarios[0].timings.total.mean.delta_percent) { throw "Nonzero over zero must be null." }
    Assert-Equal 0 $zero.scenarios[1].timings.total.mean.delta_percent "Zero over zero differs."

    Write-Aggregate $baselinePath "full" @((New-Result "long" 100 $true))
    Write-Aggregate $currentPath "full" @((New-Result "long" 110 $true))
    Write-BenchmarkComparison $baselinePath $currentPath $outputPath | Out-Null
    Assert-Equal 10 ((Get-Content $outputPath -Raw | ConvertFrom-Json).scenarios[0].timings.total.mean.delta_percent) "Long-run overall differs."

    Write-Aggregate $baselinePath "full" @((New-Result "alpha" 100))
    Write-Aggregate $currentPath "quick" @((New-Result "missing" 100))
    Assert-Throws { Write-BenchmarkComparison $baselinePath $currentPath $outputPath } "Missing scenario was accepted."
    Write-Aggregate $currentPath "quick" @((New-Result "alpha" 100), (New-Result "alpha" 100))
    Assert-Throws { Write-BenchmarkComparison $baselinePath $currentPath $outputPath } "Duplicate scenario was accepted."
    Write-Aggregate $currentPath "quick" @((New-Result "alpha" 100)) 2
    Assert-Throws { Write-BenchmarkComparison $baselinePath $currentPath $outputPath } "Aggregate schema mismatch was accepted."

    foreach ($field in @("seed", "population", "measured_ticks", "workload")) {
        $base = New-Result "alpha" 100
        $current = New-Result "alpha" 100
        if ($field -eq "workload") { $current.scenario.workload.kind = "different" } else { $current.scenario[$field]++ }
        Write-Aggregate $baselinePath "full" @($base)
        Write-Aggregate $currentPath "quick" @($current)
        Assert-Throws { Write-BenchmarkComparison $baselinePath $currentPath $outputPath } "$field mismatch was accepted."
    }
    $base = New-Result "alpha" 100
    $current = New-Result "alpha" 100
    $current.schema_version = 4
    Write-Aggregate $baselinePath "full" @($base)
    Write-Aggregate $currentPath "quick" @($current)
    Assert-Throws { Write-BenchmarkComparison $baselinePath $currentPath $outputPath } "Scenario schema mismatch was accepted."

    Write-Host "Benchmark comparison tests passed." -ForegroundColor Green
}
finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
