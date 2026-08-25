Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "benchmark-results.ps1")

function Assert-Equal {
    param($Expected, $Actual, [string]$Message)
    if ($Expected -cne $Actual) {
        throw "$Message Expected '$Expected', got '$Actual'."
    }
}

function Assert-Throws {
    param([scriptblock]$Action, [string]$Message)
    try {
        & $Action
    }
    catch {
        return
    }
    throw $Message
}

function Write-Fixture {
    param(
        [string]$Directory,
        [string]$FileName,
        [string]$ScenarioName,
        [hashtable]$Extra = @{}
    )
    $payload = [ordered]@{
        schema_version = 3
        scenario = [ordered]@{ name = $ScenarioName }
        summary = [ordered]@{ tick = 42; entities = 100 }
    }
    foreach ($entry in $Extra.GetEnumerator()) {
        $payload[$entry.Key] = $entry.Value
    }
    $json = $payload | ConvertTo-Json -Depth 100
    [System.IO.File]::WriteAllText(
        (Join-Path $Directory $FileName),
        $json,
        [System.Text.UTF8Encoding]::new($false)
    )
}

$testRoot = Join-Path ([System.IO.Path]::GetTempPath()) "nexus-benchmark-results-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $testRoot | Out-Null
try {
    Write-Fixture $testRoot "beta.json" "beta"
    Write-Fixture $testRoot "alpha.json" "alpha"
    Write-Fixture $testRoot "stale.json" "stale"

    $path = Write-BenchmarkResults -Suite "quick" -ScenarioNames @("beta", "alpha") -OutputDir $testRoot
    $aggregate = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    Assert-Equal 1 $aggregate.schema_version "Aggregate schema version differs."
    Assert-Equal "quick" $aggregate.suite "Aggregate suite differs."
    Assert-Equal 2 $aggregate.results.Count "Aggregate result count differs."
    Assert-Equal "alpha" $aggregate.results[0].scenario.name "Results are not stably ordered."
    Assert-Equal "beta" $aggregate.results[1].scenario.name "Results are not stably ordered."
    if ($aggregate.results.scenario.name -contains "stale") {
        throw "A stale scenario entered the aggregate."
    }
    $bytes = [System.IO.File]::ReadAllBytes($path)
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        throw "Aggregate JSON contains a UTF-8 BOM."
    }

    $singlePath = Write-BenchmarkResults -Suite "scenario:alpha" -ScenarioNames @("alpha") -OutputDir $testRoot
    $single = Get-Content -LiteralPath $singlePath -Raw | ConvertFrom-Json
    Assert-Equal 1 $single.results.Count "Single-scenario aggregate count differs."
    Assert-Equal "alpha" $single.results[0].scenario.name "Single-scenario aggregate differs."

    $windows = @(
        [ordered]@{ start_tick = 0; end_tick = 99 },
        [ordered]@{ start_tick = 100; end_tick = 199 }
    )
    Write-Fixture $testRoot "long-run.json" "long-run" @{
        overall = [ordered]@{ tick = 200 }
        windows = $windows
    }
    $longPath = Write-BenchmarkResults -Suite "full" -ScenarioNames @("long-run") -OutputDir $testRoot
    $long = Get-Content -LiteralPath $longPath -Raw | ConvertFrom-Json
    Assert-Equal 2 $long.results[0].windows.Count "Long-run windows were not preserved."
    Assert-Equal 100 $long.results[0].windows[1].start_tick "Long-run payload changed."

    Assert-Throws {
        Write-BenchmarkResults -Suite "quick" -ScenarioNames @("alpha", "alpha") -OutputDir $testRoot
    } "Duplicate scenarios were accepted."

    [System.IO.File]::WriteAllText((Join-Path $testRoot "invalid.json"), "not-json")
    Assert-Throws {
        Write-BenchmarkResults -Suite "quick" -ScenarioNames @("invalid") -OutputDir $testRoot
    } "Invalid JSON was accepted."

    Write-Fixture $testRoot "wrong.json" "different"
    Assert-Throws {
        Write-BenchmarkResults -Suite "quick" -ScenarioNames @("wrong") -OutputDir $testRoot
    } "A mismatched scenario name was accepted."

    Write-Host "Benchmark aggregate contract tests passed." -ForegroundColor Green
}
finally {
    $resolvedTestRoot = [System.IO.Path]::GetFullPath($testRoot)
    $resolvedTempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
    if ($resolvedTestRoot.StartsWith($resolvedTempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
    }
}
