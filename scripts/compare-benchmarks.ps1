[CmdletBinding()]
param(
    [string]$BaselinePath = (Join-Path (Split-Path -Parent $PSScriptRoot) "benchmarks/baselines/github-ubuntu-x64/benchmark-results.json"),
    [Parameter(Mandatory = $true)][string]$CurrentPath,
    [string]$OutputPath = (Join-Path (Split-Path -Parent $PSScriptRoot) "target/nexus-bench/benchmark-comparison.json")
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-comparison.ps1")

$written = Write-BenchmarkComparison `
    -BaselinePath ([System.IO.Path]::GetFullPath($BaselinePath)) `
    -CurrentPath ([System.IO.Path]::GetFullPath($CurrentPath)) `
    -OutputPath ([System.IO.Path]::GetFullPath($OutputPath))
Write-Host "Comparison: $written" -ForegroundColor Green
