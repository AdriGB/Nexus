Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "benchmark-regression-report.ps1")

$script:BenchmarkGateCandidateThresholdPercent = 30.0
$script:BenchmarkGateBenchScriptPath = Join-Path $PSScriptRoot "bench.ps1"

function Get-BenchmarkGateCandidates {
    param(
        [Parameter(Mandatory = $true)][string]$ComparisonPath,
        [double]$ThresholdPercent = $script:BenchmarkGateCandidateThresholdPercent
    )
    if (-not (Test-Path -LiteralPath $ComparisonPath -PathType Leaf)) {
        throw "Benchmark comparison '$ComparisonPath' does not exist."
    }
    try {
        $comparison = Get-Content -LiteralPath $ComparisonPath -Raw | ConvertFrom-Json
    }
    catch {
        throw "Benchmark comparison '$ComparisonPath' contains invalid JSON: $($_.Exception.Message)"
    }
    if ($comparison.schema_version -ne 1) {
        throw "Benchmark comparison '$ComparisonPath' has unsupported schema_version '$($comparison.schema_version)'."
    }

    $candidates = [System.Collections.Generic.List[object]]::new()
    foreach ($scenario in @($comparison.scenarios)) {
        $name = $scenario.name
        if ([string]::IsNullOrWhiteSpace($name)) {
            throw "Benchmark comparison contains a scenario without a name."
        }
        try {
            $mean = $scenario.timings.total.mean
            if ($mean.PSObject.Properties.Name -notcontains "delta_percent") { throw "missing total mean delta" }
            $delta = $mean.delta_percent
        }
        catch {
            throw "Scenario '$name' has malformed total mean timing data."
        }
        if ($null -eq $delta) { continue }
        if ([double]$delta -gt $ThresholdPercent) {
            $candidates.Add([pscustomobject]@{
                Name = $name
                FirstMeanDeltaPercent = [double]$delta
            })
        }
    }
    $candidates.Sort([System.Comparison[object]] {
        param($left, $right)
        return [System.StringComparer]::Ordinal.Compare($left.Name, $right.Name)
    })
    return @($candidates)
}

function Get-BenchmarkGateRetryDeltaPercent {
    param(
        [Parameter(Mandatory = $true)][string]$RetryComparisonPath,
        [Parameter(Mandatory = $true)][string]$ScenarioName
    )
    if (-not (Test-Path -LiteralPath $RetryComparisonPath -PathType Leaf)) {
        throw "Retry comparison '$RetryComparisonPath' does not exist."
    }
    try {
        $comparison = Get-Content -LiteralPath $RetryComparisonPath -Raw | ConvertFrom-Json
    }
    catch {
        throw "Retry comparison '$RetryComparisonPath' contains invalid JSON: $($_.Exception.Message)"
    }
    if ($comparison.schema_version -ne 1) {
        throw "Retry comparison '$RetryComparisonPath' has unsupported schema_version '$($comparison.schema_version)'."
    }
    foreach ($scenario in @($comparison.scenarios)) {
        if ($scenario.name -cne $ScenarioName) { continue }
        try {
            $mean = $scenario.timings.total.mean
            if ($mean.PSObject.Properties.Name -notcontains "delta_percent") { throw "missing total mean delta" }
            $delta = $mean.delta_percent
        }
        catch {
            throw "Retry comparison for scenario '$ScenarioName' has malformed total mean timing data."
        }
        if ($null -eq $delta) {
            throw "Retry comparison for scenario '$ScenarioName' has no total mean delta."
        }
        return [double]$delta
    }
    throw "Retry comparison does not contain scenario '$ScenarioName'."
}

function Invoke-BenchmarkGate {
    param(
        [Parameter(Mandatory = $true)][string]$ComparisonPath,
        [Parameter(Mandatory = $true)][string]$RetryOutputDir,
        [double]$CandidateThresholdPercent = $script:BenchmarkGateCandidateThresholdPercent,
        [AllowNull()][scriptblock]$RetryScript
    )
    $candidates = @(Get-BenchmarkGateCandidates -ComparisonPath $ComparisonPath -ThresholdPercent $CandidateThresholdPercent)
    $retry = if ($null -ne $RetryScript) {
        $RetryScript
    }
    else {
        {
            param([string]$ScenarioName, [string]$OutputDir)
            & $script:BenchmarkGateBenchScriptPath -Scenario $ScenarioName -OutputDir $OutputDir
            if ($LASTEXITCODE -ne 0) {
                throw "Benchmark retry for scenario '$ScenarioName' failed with exit code $LASTEXITCODE."
            }
        }
    }

    $retryComparisonPath = Join-Path $RetryOutputDir "benchmark-comparison.json"
    $outcomes = [System.Collections.Generic.List[object]]::new()
    foreach ($candidate in $candidates) {
        try {
            & $retry -ScenarioName $candidate.Name -OutputDir $RetryOutputDir
            $retryDelta = Get-BenchmarkGateRetryDeltaPercent `
                -RetryComparisonPath $retryComparisonPath `
                -ScenarioName $candidate.Name
            $verdict = if ($retryDelta -gt $CandidateThresholdPercent) { "Confirmed" } else { "Recovered" }
            $outcomes.Add([pscustomobject]@{
                Name = $candidate.Name
                FirstMeanDeltaPercent = $candidate.FirstMeanDeltaPercent
                RetryMeanDeltaPercent = $retryDelta
                Verdict = $verdict
                Detail = $null
            })
        }
        catch {
            $outcomes.Add([pscustomobject]@{
                Name = $candidate.Name
                FirstMeanDeltaPercent = $candidate.FirstMeanDeltaPercent
                RetryMeanDeltaPercent = $null
                Verdict = "Technical failure"
                Detail = $_.Exception.Message
            })
        }
    }

    return [pscustomobject]@{
        CandidateThresholdPercent = $CandidateThresholdPercent
        Outcomes = @($outcomes)
        HasConfirmedRegression = @($outcomes | Where-Object { $_.Verdict -eq "Confirmed" }).Count -gt 0
        HasTechnicalFailure = @($outcomes | Where-Object { $_.Verdict -eq "Technical failure" }).Count -gt 0
    }
}

function Write-GitHubBenchmarkCandidateAnnouncements {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Candidates)
    foreach ($candidate in @($Candidates)) {
        $message = "$($candidate.Name): total mean $(Format-Percent $candidate.FirstMeanDeltaPercent) exceeds the candidate threshold; a confirmation run is required before blocking."
        Write-Output "::warning title=$(ConvertTo-GitHubWorkflowCommandValue 'Performance candidate')::$(ConvertTo-GitHubWorkflowCommandValue $message)"
    }
}

function Write-GitHubBenchmarkGateOutcomeAnnotations {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Outcomes)
    foreach ($outcome in @($Outcomes)) {
        if ($outcome.Verdict -eq "Confirmed") {
            $message = "Confirmed performance regression: $($outcome.Name) total mean $(Format-Percent $outcome.RetryMeanDeltaPercent)."
            Write-Output "::error title=$(ConvertTo-GitHubWorkflowCommandValue 'Confirmed performance regression')::$(ConvertTo-GitHubWorkflowCommandValue $message)"
        }
        elseif ($outcome.Verdict -eq "Recovered") {
            $message = "$($outcome.Name): total mean $(Format-Percent $outcome.FirstMeanDeltaPercent) was not reproduced on the confirmation run ($(Format-Percent $outcome.RetryMeanDeltaPercent))."
            Write-Output "::warning title=$(ConvertTo-GitHubWorkflowCommandValue 'Performance candidate not reproduced')::$(ConvertTo-GitHubWorkflowCommandValue $message)"
        }
        else {
            $message = "$($outcome.Name): confirmation run failed technically and the gate could not classify the candidate."
            Write-Output "::error title=$(ConvertTo-GitHubWorkflowCommandValue 'Benchmark gate technical failure')::$(ConvertTo-GitHubWorkflowCommandValue $message)"
        }
    }
}

function Get-BenchmarkGateMarkdown {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Outcomes,
        [double]$CandidateThresholdPercent = $script:BenchmarkGateCandidateThresholdPercent
    )
    $thresholdText = ">" + $CandidateThresholdPercent.ToString("0.#", [System.Globalization.CultureInfo]::InvariantCulture) + "%"
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("### Performance gate")
    $lines.Add("")
    if ($Outcomes.Count -eq 0) {
        $lines.Add("No $thresholdText candidate regressions detected.")
    }
    else {
        $lines.Add("| Scenario | First mean delta | Retry mean delta | Result |")
        $lines.Add("|---|---:|---:|---|")
        foreach ($outcome in $Outcomes) {
            $retryCell = if ($null -eq $outcome.RetryMeanDeltaPercent) { "n/a" } else { Format-Percent $outcome.RetryMeanDeltaPercent }
            $lines.Add("| $($outcome.Name) | $(Format-Percent $outcome.FirstMeanDeltaPercent) | $retryCell | $($outcome.Verdict) |")
        }
        $confirmedCount = @($Outcomes | Where-Object { $_.Verdict -eq "Confirmed" }).Count
        $lines.Add("")
        if ($confirmedCount -eq 0) {
            $lines.Add("No confirmed performance regressions.")
        }
        else {
            $lines.Add("$confirmedCount confirmed performance regression(s); the job fails until the baseline is updated through a deliberate PR.")
        }
    }
    $lines.Add("")
    $lines.Add("Performance gates require reproduction on a second run: every $thresholdText candidate is re-measured exactly once in isolation against the same baseline before blocking. Shared GitHub-hosted runners are noisy, so one confirmation is an operational policy, not a statistical guarantee.")
    return $lines -join [Environment]::NewLine
}
