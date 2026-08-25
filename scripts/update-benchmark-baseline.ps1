[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$InputPath,
    [string]$OutputPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "benchmarks/baselines/github-ubuntu-x64/benchmark-results.json")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-comparison.ps1")

$input = Read-BenchmarkAggregate ([System.IO.Path]::GetFullPath($InputPath))
if ($input.Aggregate.suite -cne "full") {
    throw "A baseline update requires suite 'full', got '$($input.Aggregate.suite)'."
}
$expected = @(
    "baseline-100", "baseline-1000", "baseline-10000", "dense-social-1000",
    "households-1000", "long-run-1000", "pathfinding-heavy-1000", "scarcity-1000"
)
$actual = @($input.ByName.Keys | Sort-Object)
if (($expected -join "`n") -cne ($actual -join "`n")) {
    throw "Full baseline scenarios differ from the registered set. Expected: $($expected -join ', '). Actual: $($actual -join ', ')."
}

$resolvedOutput = [System.IO.Path]::GetFullPath($OutputPath)
$parent = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$json = $input.Aggregate | ConvertTo-Json -Depth 100
[System.IO.File]::WriteAllText(
    $resolvedOutput,
    $json + [Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Host "Baseline updated: $resolvedOutput" -ForegroundColor Green
