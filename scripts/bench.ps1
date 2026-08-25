[CmdletBinding(DefaultParameterSetName = "Suite")]
param(
    [Parameter(ParameterSetName = "Suite")]
    [ValidateSet("quick", "micro", "scenarios", "full")]
    [string]$Suite = "quick",

    [Parameter(Mandatory = $true, ParameterSetName = "Scenario")]
    [ValidateNotNullOrEmpty()]
    [string]$Scenario,

    [string]$OutputDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$engineRoot = Join-Path $repoRoot "engine"
. (Join-Path $PSScriptRoot "benchmark-results.ps1")
. (Join-Path $PSScriptRoot "benchmark-comparison.ps1")
. (Join-Path $PSScriptRoot "benchmark-regression-report.ps1")
if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $repoRoot "target/nexus-bench"
}
$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)
$aggregatePath = Join-Path $OutputDir "benchmark-results.json"
$comparisonPath = Join-Path $OutputDir "benchmark-comparison.json"
if (Test-Path -LiteralPath $aggregatePath -PathType Leaf) {
    Remove-Item -LiteralPath $aggregatePath -Force
}
if (Test-Path -LiteralPath $comparisonPath -PathType Leaf) {
    Remove-Item -LiteralPath $comparisonPath -Force
}

function Invoke-NativeStep {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )
    Write-Host "`n==> $Name" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

function Build-BenchmarkRunner {
    Push-Location $engineRoot
    try {
        Invoke-NativeStep "Building benchmark runner" {
            cargo build --release --features benchmarks --bin nexus-bench
        }
        $metadataText = cargo metadata --format-version 1 --no-deps
        if ($LASTEXITCODE -ne 0) {
            throw "cargo metadata failed with exit code $LASTEXITCODE."
        }
        $targetDirectory = ($metadataText | ConvertFrom-Json).target_directory
        $binaryName = if ($env:OS -eq "Windows_NT") { "nexus-bench.exe" } else { "nexus-bench" }
        $runner = Join-Path $targetDirectory "release/$binaryName"
        if (-not (Test-Path -LiteralPath $runner -PathType Leaf)) {
            throw "Benchmark runner was not found at '$runner'."
        }
        return $runner
    }
    finally {
        Pop-Location
    }
}

function Get-ScenarioNames {
    param([Parameter(Mandatory = $true)][string]$Runner)
    $names = @(& $Runner --list)
    if ($LASTEXITCODE -ne 0 -or $names.Count -eq 0) {
        throw "Could not obtain benchmark scenarios from nexus-bench."
    }
    return $names
}

function Invoke-Scenario {
    param(
        [Parameter(Mandatory = $true)][string]$Runner,
        [Parameter(Mandatory = $true)][string]$Name
    )
    Write-Host "`n==> Scenario $Name" -ForegroundColor Cyan
    $lines = @(& $Runner $Name)
    if ($LASTEXITCODE -ne 0) {
        throw "Scenario '$Name' failed with exit code $LASTEXITCODE."
    }
    $json = ($lines -join [Environment]::NewLine).Trim()
    if ([string]::IsNullOrWhiteSpace($json)) {
        throw "Scenario '$Name' produced empty stdout."
    }
    try {
        $payload = $json | ConvertFrom-Json
    }
    catch {
        throw "Scenario '$Name' produced invalid JSON: $($_.Exception.Message)"
    }
    if ($null -eq $payload.schema_version) {
        throw "Scenario '$Name' result has no schema_version."
    }
    if ($payload.scenario.name -ne $Name) {
        throw "Scenario result name '$($payload.scenario.name)' does not match '$Name'."
    }
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
    $path = Join-Path $OutputDir "$Name.json"
    [System.IO.File]::WriteAllText(
        $path,
        $json + [Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Invoke-Criterion {
    param([Parameter(Mandatory = $true)][bool]$Quick)
    Push-Location $engineRoot
    try {
        foreach ($target in @("pathfinding", "spatial", "autonomy")) {
            if ($Quick) {
                Invoke-NativeStep "Criterion $target (quick)" {
                    cargo bench --features benchmarks --bench $target -- --quick
                }
            }
            else {
                Invoke-NativeStep "Criterion $target" {
                    cargo bench --features benchmarks --bench $target
                }
            }
        }
    }
    finally {
        Pop-Location
    }
}

$runner = $null
$availableScenarios = @()
$selectedScenarios = @()
$runCriterion = $false
$quickCriterion = $false

if ($PSCmdlet.ParameterSetName -eq "Scenario") {
    $runner = Build-BenchmarkRunner
    $availableScenarios = @(Get-ScenarioNames -Runner $runner)
    if ($Scenario -notin $availableScenarios) {
        throw "Unknown scenario '$Scenario'. Available scenarios: $($availableScenarios -join ', ')."
    }
    $selectedScenarios = @($Scenario)
    $label = "scenario:$Scenario"
}
else {
    $label = $Suite
    switch ($Suite) {
        "quick" {
            $selectedScenarios = @("baseline-100", "baseline-1000")
            $runCriterion = $true
            $quickCriterion = $true
        }
        "micro" {
            $runCriterion = $true
        }
        "scenarios" {
            $runner = Build-BenchmarkRunner
            $availableScenarios = @(Get-ScenarioNames -Runner $runner)
            $selectedScenarios = @($availableScenarios | Where-Object { $_ -ne "long-run-1000" })
        }
        "full" {
            $runner = Build-BenchmarkRunner
            $availableScenarios = @(Get-ScenarioNames -Runner $runner)
            $selectedScenarios = $availableScenarios
            $runCriterion = $true
        }
    }
}

if ($selectedScenarios.Count -gt 0 -and $null -eq $runner) {
    $runner = Build-BenchmarkRunner
    $availableScenarios = @(Get-ScenarioNames -Runner $runner)
}
foreach ($name in $selectedScenarios) {
    if ($name -notin $availableScenarios) {
        throw "Required scenario '$name' is not registered. Available scenarios: $($availableScenarios -join ', ')."
    }
    Invoke-Scenario -Runner $runner -Name $name
}
if ($selectedScenarios.Count -gt 0) {
    $aggregatePath = Write-BenchmarkResults `
        -Suite $label `
        -ScenarioNames $selectedScenarios `
        -OutputDir $OutputDir
    Write-Host "Aggregate: $aggregatePath"
    $baselinePath = Join-Path $repoRoot "benchmarks/baselines/github-ubuntu-x64/benchmark-results.json"
    if (Test-Path -LiteralPath $baselinePath -PathType Leaf) {
        $comparisonPath = Write-BenchmarkComparison `
            -BaselinePath $baselinePath `
            -CurrentPath $aggregatePath `
            -OutputPath $comparisonPath
        Write-Host "Comparison: $comparisonPath"
        $slowdowns = @(Get-ReportableBenchmarkSlowdowns -ComparisonPath $comparisonPath)
        Write-InformationalBenchmarkReport -Slowdowns $slowdowns
    }
}
else {
    Write-Host "No end-to-end scenario results to aggregate for suite '$label'."
}
if ($runCriterion) {
    Invoke-Criterion -Quick $quickCriterion
}

Write-Host "`nBenchmark suite '$label' completed." -ForegroundColor Green
if ($selectedScenarios.Count -gt 0) {
    Write-Host "Results: $OutputDir"
}
