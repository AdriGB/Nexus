Set-StrictMode -Version Latest

$script:InformationalSlowdownThresholdPercent = 10.0
$script:WarningSlowdownThresholdPercent = 20.0

function Get-BenchmarkPerformanceObservations {
    param([Parameter(Mandatory = $true)][string]$ComparisonPath)
    if (-not (Test-Path -LiteralPath $ComparisonPath -PathType Leaf)) { throw "Benchmark comparison '$ComparisonPath' does not exist." }
    try { $comparison = Get-Content -LiteralPath $ComparisonPath -Raw | ConvertFrom-Json }
    catch { throw "Benchmark comparison '$ComparisonPath' contains invalid JSON: $($_.Exception.Message)" }
    if ($comparison.schema_version -ne 1) { throw "Benchmark comparison '$ComparisonPath' has unsupported schema_version '$($comparison.schema_version)'." }

    $observations = [System.Collections.Generic.List[object]]::new()
    foreach ($scenario in @($comparison.scenarios)) {
        $name = $scenario.name
        if ([string]::IsNullOrWhiteSpace($name)) { throw "Benchmark comparison contains a scenario without a name." }
        try {
            $mean = $scenario.timings.total.mean
            $p95 = $scenario.timings.total.p95
            foreach ($value in @($mean.baseline_us, $mean.current_us, $p95.delta_percent)) { if ($null -eq $value) { throw "missing timing value" } }
            $meanDelta = $mean.delta_percent
        }
        catch { throw "Scenario '$name' has malformed total mean or p95 timing data." }
        if ($null -eq $meanDelta -or [double]$meanDelta -le $script:InformationalSlowdownThresholdPercent) { continue }
        $level = if ([double]$meanDelta -gt $script:WarningSlowdownThresholdPercent) { "Warning" } else { "Informational" }
        $observations.Add([pscustomobject]@{
            Level = $level; Name = $name; BaselineMeanUs = [double]$mean.baseline_us; CurrentMeanUs = [double]$mean.current_us
            MeanDeltaPercent = [double]$meanDelta; P95DeltaPercent = [double]$p95.delta_percent
        })
    }
    $observations.Sort([System.Comparison[object]]{
        param($left, $right)
        $levelOrder = @{"Warning" = 0; "Informational" = 1}
        $severityOrder = $levelOrder[$left.Level].CompareTo($levelOrder[$right.Level])
        if ($severityOrder -ne 0) { return $severityOrder }
        $deltaOrder = $right.MeanDeltaPercent.CompareTo($left.MeanDeltaPercent)
        if ($deltaOrder -ne 0) { return $deltaOrder }
        return [System.StringComparer]::Ordinal.Compare($left.Name, $right.Name)
    })
    return @($observations)
}

function Get-ReportableBenchmarkSlowdowns { param([Parameter(Mandatory = $true)][string]$ComparisonPath) return @(Get-BenchmarkPerformanceObservations -ComparisonPath $ComparisonPath) }
function Format-Percent { param([double]$Value) ($Value.ToString("+0.00;-0.00;0.00", [System.Globalization.CultureInfo]::InvariantCulture) + "%") }
function Format-Milliseconds { param([double]$Microseconds) (($Microseconds / 1000.0).ToString("0.00", [System.Globalization.CultureInfo]::InvariantCulture) + " ms") }

function Write-InformationalBenchmarkReport {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Slowdowns)
    Write-Host "`n==> Performance report" -ForegroundColor Cyan
    if ($Slowdowns.Count -eq 0) { Write-Host "No performance slowdowns above 10%."; return }
    foreach ($item in $Slowdowns) {
        $prefix = if ($item.Level -eq "Warning") { "WARNING" } else { "INFO" }
        Write-Host "${prefix}: $($item.Name) total mean $(Format-Percent $item.MeanDeltaPercent)"
    }
    $warningCount = @($Slowdowns | Where-Object Level -eq "Warning").Count
    if ($warningCount -gt 0) { Write-Host "$warningCount warning(s) above 20%. Job status is unaffected." }
    Write-Host "Observations are informational only and may include runner noise."
}

function Get-InformationalBenchmarkMarkdown {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Slowdowns)
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("### Performance observations"); $lines.Add("")
    if ($Slowdowns.Count -eq 0) { $lines.Add("No performance slowdowns above 10%.") }
    else {
        $lines.Add("| Level | Scenario | Baseline mean | Current mean | Mean delta | p95 delta |"); $lines.Add("|---|---|---:|---:|---:|---:|")
        foreach ($item in $Slowdowns) {
            $level = if ($item.Level -eq "Warning") { "Warning" } else { "Info" }
            $lines.Add("| $level | $($item.Name) | $(Format-Milliseconds $item.BaselineMeanUs) | $(Format-Milliseconds $item.CurrentMeanUs) | $(Format-Percent $item.MeanDeltaPercent) | $(Format-Percent $item.P95DeltaPercent) |")
        }
    }
    $lines.Add(""); $lines.Add("Informational only — this does not affect job status. The 10% and 20% levels may include runner noise.")
    return $lines -join [Environment]::NewLine
}

function ConvertTo-GitHubWorkflowCommandValue {
    param([Parameter(Mandatory = $true)][string]$Value)
    return $Value.Replace("%", "%25").Replace("`r", "%0D").Replace("`n", "%0A").Replace(":", "%3A").Replace(",", "%2C")
}

function Write-GitHubBenchmarkWarningAnnotations {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Observations,
        [double]$ExcludeCandidatesAbovePercent = 0
    )
    foreach ($item in @($Observations | Where-Object {
        $_.Level -eq "Warning" -and
        ($ExcludeCandidatesAbovePercent -le 0 -or [double]$_.MeanDeltaPercent -le $ExcludeCandidatesAbovePercent)
    })) {
        $message = "$($item.Name): total mean $(Format-Percent $item.MeanDeltaPercent) ($(Format-Milliseconds $item.BaselineMeanUs) -> $(Format-Milliseconds $item.CurrentMeanUs))"
        Write-Output "::warning title=$(ConvertTo-GitHubWorkflowCommandValue 'Potential performance slowdown')::$(ConvertTo-GitHubWorkflowCommandValue $message)"
    }
}
