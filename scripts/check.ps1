[CmdletBinding()]
param(
    [ValidateSet("all", "engine", "web")]
    [string]$Target = "all"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot

function Invoke-CheckStep {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [scriptblock]$Command
    )

    Write-Host "`n==> $Name" -ForegroundColor Cyan
    & $Command

    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE."
    }
}

function Test-Engine {
    Push-Location (Join-Path $repoRoot "engine")
    try {
        Invoke-CheckStep "Rust format" { cargo fmt --check }
        Invoke-CheckStep "Rust clippy (native)" { cargo clippy -- -D warnings }
        Invoke-CheckStep "Rust tests" { cargo test }
        Invoke-CheckStep "Benchmark runner clippy" {
            cargo clippy --features benchmarks --lib --bin nexus-bench --bench pathfinding --bench spatial --bench autonomy -- -D warnings
        }
        Invoke-CheckStep "Benchmark targets compile" {
            cargo check --benches --features benchmarks
        }
        Invoke-CheckStep "Benchmark runner tests" {
            cargo test --features benchmarks benchmarking::
        }
        Invoke-CheckStep "Benchmark aggregate contract tests" {
            & (Join-Path $repoRoot "scripts/test-benchmark-results.ps1")
        }
        Invoke-CheckStep "Benchmark comparison tests" {
            & (Join-Path $repoRoot "scripts/test-benchmark-comparison.ps1")
        }
        Invoke-CheckStep "Rust clippy (WASM)" {
            cargo clippy --target wasm32-unknown-unknown -- -D warnings
        }
        Invoke-CheckStep "WASM build" {
            wasm-pack build --target web --out-dir ../web/src/wasm
        }
    }
    finally {
        Pop-Location
    }
}

function Test-Web {
    Push-Location (Join-Path $repoRoot "web")
    try {
        if (-not (Test-Path "src/wasm")) {
            throw "web/src/wasm is missing. Run the engine checks first."
        }

        Invoke-CheckStep "Install web dependencies" { npm ci }
        Invoke-CheckStep "TypeScript typecheck" { npm run typecheck }
        Invoke-CheckStep "Web tests" { npm test }
        Invoke-CheckStep "Web build" { npm run build }
    }
    finally {
        Pop-Location
    }
}

if ($Target -in @("all", "engine")) {
    Test-Engine
}

if ($Target -in @("all", "web")) {
    Test-Web
}

Write-Host "`nAll '$Target' checks passed." -ForegroundColor Green
