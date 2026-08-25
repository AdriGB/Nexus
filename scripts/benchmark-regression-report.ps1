Set-StrictMode -Version Latest

$script:InformationalSlowdownThresholdPercent = 10.0

function Get-ReportableBenchmarkSlowdowns {
    param([Parameter(Mandatory = $true)][string]$ComparisonPath)
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

    $reportable = [System.Collections.Generic.List[object]]::new()
    foreach ($scenario in @($comparison.scenarios)) {
        $name = $scenario.name
        if ([string]::IsNullOrWhiteSpace($name)) {
            throw "Benchmark comparison contains a scenario without a name."
        }
        try {
            $mean = $scenario.timings.total.mean
            $p95 = $scenario.timings.total.p95
            foreach ($value in @($mean.baseline_us, $mean.current_us, $p95.delta_percent)) {
                if ($null -eq $value) { throw "missing timing value" }
            }
            $meanDelta = $mean.delta_percent
        }
        catch {
            throw "Scenario '$name' has malformed total mean or p95 timing data."
        }
        if ($null -ne $meanDelta -and [double]$meanDelta -gt $script:InformationalSlowdownThresholdPercent) {
            $reportable.Add([pscustomobject]@{
                Name = $name
                BaselineMeanUs = [double]$mean.baseline_us
                CurrentMeanUs = [double]$mean.current_us
                MeanDeltaPercent = [double]$meanDelta
                P95DeltaPercent = [double]$p95.delta_percent
            })
        }
    }

    $reportable.Sort([System.Comparison[object]]{
        param($left, $right)
        $deltaOrder = $right.MeanDeltaPercent.CompareTo($left.MeanDeltaPercent)
        if ($deltaOrder -ne 0) { return $deltaOrder }
        return [System.StringComparer]::Ordinal.Compare($left.Name, $right.Name)
    })
    return @($reportable)
}

function Format-Percent {
    param([double]$Value)
    return $Value.ToString("+0.00;-0.00;0.00", [System.Globalization.CultureInfo]::InvariantCulture) + "%"
}

function Format-Milliseconds {
    param([double]$Microseconds)
    return ($Microseconds / 1000.0).ToString("0.00", [System.Globalization.CultureInfo]::InvariantCulture) + " ms"
}

function Write-InformationalBenchmarkReport {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Slowdowns)
    Write-Host "`n==> Informational performance report" -ForegroundColor Cyan
    if ($Slowdowns.Count -eq 0) {
        Write-Host "No informational slowdowns above 10%."
        return
    }
    foreach ($item in $Slowdowns) {
        Write-Host "$($item.Name): total mean $(Format-Percent $item.MeanDeltaPercent)"
    }
    Write-Host "$($Slowdowns.Count) scenario(s) above 10%. Informational only; job status is unaffected."
}

function Get-InformationalBenchmarkMarkdown {
    param([Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Slowdowns)
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add("### Informational slowdowns (>10% total mean)")
    $lines.Add("")
    if ($Slowdowns.Count -eq 0) {
        $lines.Add("None.")
    }
    else {
        $lines.Add("| Scenario | Baseline mean | Current mean | Mean delta | p95 delta |")
        $lines.Add("|---|---:|---:|---:|---:|")
        foreach ($item in $Slowdowns) {
            $lines.Add("| $($item.Name) | $(Format-Milliseconds $item.BaselineMeanUs) | $(Format-Milliseconds $item.CurrentMeanUs) | $(Format-Percent $item.MeanDeltaPercent) | $(Format-Percent $item.P95DeltaPercent) |")
        }
    }
    $lines.Add("")
    $lines.Add("Informational only — this does not affect job status. The 10% level may include runner noise.")
    return $lines -join [Environment]::NewLine
}
