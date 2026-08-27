Set-StrictMode -Version Latest

$script:InformationalSlowdownThresholdPercent = 10.0
$script:WarningSlowdownThresholdPercent = 20.0
$script:AttributionLimit = 3

function Get-OptionalProperty {
    param($Object, [string]$Name)
    if ($null -eq $Object) { return $null }
    if ($Object -is [System.Collections.IDictionary]) {
        if ($Object.Contains($Name)) { return $Object[$Name] }
        return $null
    }
    if (@($Object.PSObject.Properties.Name) -contains $Name) { return $Object.$Name }
    return $null
}

function Get-PropertyNames {
    param($Object)
    if ($null -eq $Object) { return @() }
    if ($Object -is [System.Collections.IDictionary]) {
        return @($Object.Keys | ForEach-Object { [string]$_ })
    }
    return @($Object.PSObject.Properties.Name)
}

function Get-NamedContainerEntries {
    param($Container)
    $entries = [System.Collections.Generic.List[object]]::new()
    if ($null -eq $Container) { return @() }
    if ($Container -is [System.Collections.IDictionary]) {
        foreach ($key in @($Container.Keys)) {
            $entries.Add([pscustomobject]@{ Name = [string]$key; Value = $Container[$key] })
        }
    }
    else {
        foreach ($property in $Container.PSObject.Properties) {
            $entries.Add([pscustomobject]@{ Name = $property.Name; Value = $property.Value })
        }
    }
    return @($entries)
}

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
        $attribution = Get-BenchmarkRegressionAttribution -Scenario $scenario
        $observations.Add([pscustomobject]@{
            Level = $level
            Name = $name
            BaselineMeanUs = [double]$mean.baseline_us
            CurrentMeanUs = [double]$mean.current_us
            MeanDeltaPercent = [double]$meanDelta
            P95DeltaPercent = [double]$p95.delta_percent
            Attribution = $attribution
            Explanation = Format-BenchmarkAttribution -Attribution $attribution
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

function Sort-AttributionItems {
    param($Items)
    $list = [System.Collections.Generic.List[object]]::new()
    foreach ($item in @($Items)) {
        if ($null -ne $item) { $list.Add($item) }
    }
    $list.Sort([System.Comparison[object]] {
        param($left, $right)
        $deltaOrder = $right.AbsoluteDelta.CompareTo($left.AbsoluteDelta)
        if ($deltaOrder -ne 0) { return $deltaOrder }
        return [System.StringComparer]::Ordinal.Compare($left.Name, $right.Name)
    })
    return @($list)
}

function Get-CountAttributionItems {
    param($Container, [string]$Kind)
    $items = [System.Collections.Generic.List[object]]::new()
    foreach ($entry in @(Get-NamedContainerEntries $Container)) {
        $value = $entry.Value
        if ($null -eq $value) { continue }
        $names = @(Get-PropertyNames $value)
        if ($names -notcontains "baseline" -or $names -notcontains "current") { continue }
        $baseline = [double](Get-OptionalProperty $value "baseline")
        $current = [double](Get-OptionalProperty $value "current")
        $absolute = $current - $baseline
        if ($absolute -le 0) { continue }
        $delta = Get-OptionalProperty $value "delta_percent"
        $items.Add([pscustomobject]@{
            Name = $entry.Name
            Kind = $Kind
            AbsoluteDelta = $absolute
            DeltaPercent = $delta
            Unit = "count"
        })
    }
    return @(Sort-AttributionItems $items)
}

function Get-BenchmarkRegressionAttribution {
    param(
        [Parameter(Mandatory = $true)]$Scenario,
        [int]$Limit = $script:AttributionLimit
    )
    $phases = [System.Collections.Generic.List[object]]::new()
    foreach ($entry in @(Get-NamedContainerEntries (Get-OptionalProperty $Scenario "timings"))) {
        if ($entry.Name -eq "total") { continue }
        $mean = Get-OptionalProperty $entry.Value "mean"
        if ($null -eq $mean) { continue }
        $names = @(Get-PropertyNames $mean)
        if ($names -notcontains "baseline_us" -or $names -notcontains "current_us") { continue }
        $baseline = [double](Get-OptionalProperty $mean "baseline_us")
        $current = [double](Get-OptionalProperty $mean "current_us")
        $absolute = $current - $baseline
        if ($absolute -le 0) { continue }
        $delta = Get-OptionalProperty $mean "delta_percent"
        $phases.Add([pscustomobject]@{
            Name = $entry.Name
            Kind = "phase"
            AbsoluteDelta = $absolute
            DeltaPercent = $delta
            Unit = "us"
        })
    }

    return [pscustomobject]@{
        Phases = @(Sort-AttributionItems $phases | Select-Object -First $Limit)
        Work = @(Get-CountAttributionItems -Container (Get-OptionalProperty $Scenario "work") -Kind "work" | Select-Object -First $Limit)
        State = @(Get-CountAttributionItems -Container (Get-OptionalProperty $Scenario "state_peak") -Kind "state" | Select-Object -First $Limit)
    }
}

function Format-SignedMicroseconds {
    param([double]$Value)
    return ($Value.ToString("+0.00;-0.00;0.00", [System.Globalization.CultureInfo]::InvariantCulture) + " us")
}

function Format-SignedCount {
    param([double]$Value)
    return $Value.ToString("+0;-0;0", [System.Globalization.CultureInfo]::InvariantCulture)
}

function Format-BenchmarkAttribution {
    param($Attribution)
    if ($null -eq $Attribution) { return "" }
    $parts = [System.Collections.Generic.List[string]]::new()
    foreach ($item in @($Attribution.Phases)) {
        if ($null -eq $item) { continue }
        $parts.Add("$($item.Name) $(Format-SignedMicroseconds $item.AbsoluteDelta)")
    }
    foreach ($item in @($Attribution.Work)) {
        if ($null -eq $item) { continue }
        $parts.Add("$($item.Name) $(Format-SignedCount $item.AbsoluteDelta)")
    }
    foreach ($item in @($Attribution.State)) {
        if ($null -eq $item) { continue }
        $parts.Add("$($item.Name) $(Format-SignedCount $item.AbsoluteDelta)")
    }
    return ($parts -join "; ")
}

function Format-AttributionSuffix {
    param([string]$Explanation)
    if ([string]::IsNullOrWhiteSpace($Explanation)) { return "" }
    return "; $Explanation"
}

function Get-ScenarioAttributionExplanation {
    param($Comparison, [string]$ScenarioName)
    if ($null -eq $Comparison) { return "" }
    foreach ($scenario in @($Comparison.scenarios)) {
        if ($scenario.name -cne $ScenarioName) { continue }
        return Format-BenchmarkAttribution (Get-BenchmarkRegressionAttribution -Scenario $scenario)
    }
    return ""
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
        Write-Host "${prefix}: $($item.Name) total mean $(Format-Percent $item.MeanDeltaPercent)$(Format-AttributionSuffix $item.Explanation)"
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
        $lines.Add("| Level | Scenario | Baseline mean | Current mean | Mean delta | p95 delta | Explained by |")
        $lines.Add("|---|---|---:|---:|---:|---:|---|")
        foreach ($item in $Slowdowns) {
            $level = if ($item.Level -eq "Warning") { "Warning" } else { "Info" }
            $explained = if ([string]::IsNullOrWhiteSpace($item.Explanation)) { "—" } else { $item.Explanation }
            $lines.Add("| $level | $($item.Name) | $(Format-Milliseconds $item.BaselineMeanUs) | $(Format-Milliseconds $item.CurrentMeanUs) | $(Format-Percent $item.MeanDeltaPercent) | $(Format-Percent $item.P95DeltaPercent) | $explained |")
        }
    }
    $lines.Add(""); $lines.Add("Informational only — this does not affect job status. The 10% and 20% levels may include runner noise. Phase and counter attribution ranks absolute increases and never changes job status.")
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
        $message = "$($item.Name): total mean $(Format-Percent $item.MeanDeltaPercent) ($(Format-Milliseconds $item.BaselineMeanUs) -> $(Format-Milliseconds $item.CurrentMeanUs))$(Format-AttributionSuffix $item.Explanation)"
        Write-Output "::warning title=$(ConvertTo-GitHubWorkflowCommandValue 'Potential performance slowdown')::$(ConvertTo-GitHubWorkflowCommandValue $message)"
    }
}
