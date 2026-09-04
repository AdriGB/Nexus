Set-StrictMode -Version Latest

$script:BenchmarkComparisonSchemaVersion = 1
$script:TimingCategories = @(
    "total", "world_maintenance", "physiology", "dependent_care", "households",
    "spatial_index", "autonomy", "survival", "mortality", "lifecycle",
    "relationships", "reproduction"
)
$script:TimingStatistics = [ordered]@{
    mean = "mean_us"
    median = "median_us"
    p95 = "p95_us"
    p99 = "p99_us"
    max = "max_us"
}
$script:WorkCounters = @(
    "entities_processed", "entities_perceived", "goal_evaluations", "goal_changes",
    "plans_created", "actions_executed", "social_interactions", "spatial_queries",
    "pathfinding_searches", "pathfinding_nodes_expanded", "events_created",
    "orphan_reassignment_scans", "household_sync_scans", "household_migration_scans", "conception_scans"
)
$script:StateGauges = @(
    "entities_alive", "known_entities_total", "known_entities_max_per_entity",
    "known_resources_total", "known_resources_max_per_entity", "known_dead_entities_total",
    "active_grief_states", "recent_events_len", "recent_events_capacity",
    "households_active", "genealogy_links"
)

function Read-BenchmarkAggregate {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Benchmark aggregate '$Path' does not exist."
    }
    try {
        $aggregate = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    }
    catch {
        throw "Benchmark aggregate '$Path' contains invalid JSON: $($_.Exception.Message)"
    }
    if ($aggregate.schema_version -ne 1) {
        throw "Benchmark aggregate '$Path' has unsupported schema_version '$($aggregate.schema_version)'."
    }
    if ([string]::IsNullOrWhiteSpace($aggregate.suite)) {
        throw "Benchmark aggregate '$Path' has no suite."
    }
    $byName = [ordered]@{}
    foreach ($result in @($aggregate.results)) {
        $name = $result.scenario.name
        if ([string]::IsNullOrWhiteSpace($name)) {
            throw "Benchmark aggregate '$Path' contains a result without scenario.name."
        }
        if ($byName.Contains($name)) {
            throw "Benchmark aggregate '$Path' contains duplicate scenario '$name'."
        }
        $byName[$name] = $result
    }
    return [pscustomobject]@{ Aggregate = $aggregate; ByName = $byName }
}

function Assert-CompatibleScenario {
    param($Baseline, $Current, [string]$Name)
    if ($Baseline.schema_version -ne $Current.schema_version) {
        throw "Scenario '$Name' is incompatible with baseline: scenario schema_version differs ($($Baseline.schema_version) vs $($Current.schema_version))."
    }
    if ($Baseline.PSObject.Properties.Name -notcontains "state_hash" -or [string]::IsNullOrWhiteSpace($Baseline.state_hash)) {
        throw "Scenario '$Name' is incompatible with baseline: baseline is missing state_hash."
    }
    if ($Current.PSObject.Properties.Name -notcontains "state_hash" -or [string]::IsNullOrWhiteSpace($Current.state_hash)) {
        throw "Scenario '$Name' is incompatible with baseline: current run is missing state_hash."
    }
    if ($Baseline.state_hash -cne $Current.state_hash) {
        throw "Scenario '$Name' state_hash mismatch: baseline $($Baseline.state_hash) != current $($Current.state_hash)."
    }
    foreach ($field in @("seed", "population", "warmup_ticks", "measured_ticks", "world", "workload")) {
        $baselineValue = $Baseline.scenario.$field | ConvertTo-Json -Compress -Depth 100
        $currentValue = $Current.scenario.$field | ConvertTo-Json -Compress -Depth 100
        if ($baselineValue -cne $currentValue) {
            throw "Scenario '$Name' is incompatible with baseline: $field differs ($baselineValue vs $currentValue)."
        }
    }
}

function Get-TimingSummary {
    param($Result, [string]$Name)
    if ($Result.PSObject.Properties.Name -contains "summary") {
        return $Result.summary
    }
    if ($Result.PSObject.Properties.Name -contains "overall") {
        return $Result.overall
    }
    throw "Scenario '$Name' has no summary or long-run overall timing payload."
}

function Get-DeltaPercent {
    param([double]$Baseline, [double]$Current)
    if ($Baseline -eq 0) {
        if ($Current -eq 0) { return 0.0 }
        return $null
    }
    return [math]::Round((($Current - $Baseline) / $Baseline) * 100.0, 6)
}

function Get-RequiredCountMap {
    param($Summary, [string]$ScenarioName, [string]$Field, [string[]]$Names)
    if ($Summary.PSObject.Properties.Name -notcontains $Field -or $null -eq $Summary.$Field) {
        throw "Scenario '$ScenarioName' has no $Field."
    }
    $source = $Summary.$Field
    $map = [ordered]@{}
    foreach ($name in $Names) {
        if ($source.PSObject.Properties.Name -notcontains $name) {
            throw "Scenario '$ScenarioName' $Field has no '$name'."
        }
        $map[$name] = [double]$source.$name
    }
    return $map
}

function ConvertTo-CountComparison {
    param($BaselineMap, $CurrentMap)
    $comparison = [ordered]@{}
    foreach ($name in $BaselineMap.Keys) {
        $baselineValue = $BaselineMap[$name]
        $currentValue = $CurrentMap[$name]
        $comparison[$name] = [ordered]@{
            baseline = $baselineValue
            current = $currentValue
            delta_percent = Get-DeltaPercent $baselineValue $currentValue
        }
    }
    return $comparison
}

function Write-BenchmarkComparison {
    param(
        [Parameter(Mandatory = $true)][string]$BaselinePath,
        [Parameter(Mandatory = $true)][string]$CurrentPath,
        [Parameter(Mandatory = $true)][string]$OutputPath
    )
    $baselineData = Read-BenchmarkAggregate $BaselinePath
    $currentData = Read-BenchmarkAggregate $CurrentPath
    $names = [System.Collections.Generic.List[string]]::new()
    foreach ($name in $currentData.ByName.Keys) { $names.Add($name) }
    $names.Sort([System.StringComparer]::Ordinal)

    $scenarios = @()
    $deltas = [System.Collections.Generic.List[double]]::new()
    foreach ($name in $names) {
        if (-not $baselineData.ByName.Contains($name)) {
            throw "Current scenario '$name' does not exist in the baseline."
        }
        $baselineResult = $baselineData.ByName[$name]
        $currentResult = $currentData.ByName[$name]
        Assert-CompatibleScenario $baselineResult $currentResult $name
        $baselineSummary = Get-TimingSummary $baselineResult $name
        $currentSummary = Get-TimingSummary $currentResult $name
        $timings = [ordered]@{}
        foreach ($category in $script:TimingCategories) {
            if ($baselineSummary.PSObject.Properties.Name -notcontains $category -or
                $currentSummary.PSObject.Properties.Name -notcontains $category) {
                throw "Scenario '$name' has no canonical timing category '$category'."
            }
            $stats = [ordered]@{}
            foreach ($statistic in $script:TimingStatistics.GetEnumerator()) {
                $field = $statistic.Value
                if ($baselineSummary.$category.PSObject.Properties.Name -notcontains $field -or
                    $currentSummary.$category.PSObject.Properties.Name -notcontains $field) {
                    throw "Scenario '$name' timing '$category' has no '$field'."
                }
                $baselineValue = [double]$baselineSummary.$category.$field
                $currentValue = [double]$currentSummary.$category.$field
                $delta = Get-DeltaPercent $baselineValue $currentValue
                if ($null -ne $delta) { $deltas.Add($delta) }
                $stats[$statistic.Key] = [ordered]@{
                    baseline_us = $baselineValue
                    current_us = $currentValue
                    delta_percent = $delta
                }
            }
            $timings[$category] = $stats
        }
        $work = ConvertTo-CountComparison `
            (Get-RequiredCountMap $baselineSummary $name "work_total" $script:WorkCounters) `
            (Get-RequiredCountMap $currentSummary $name "work_total" $script:WorkCounters)
        $statePeak = ConvertTo-CountComparison `
            (Get-RequiredCountMap $baselineSummary $name "state_peak" $script:StateGauges) `
            (Get-RequiredCountMap $currentSummary $name "state_peak" $script:StateGauges)
        $scenarios += [ordered]@{
            name = $name
            state_hash = $currentResult.state_hash
            timings = $timings
            work = $work
            state_peak = $statePeak
        }
    }

    $largestPositive = if ($deltas.Count -eq 0) { $null } else { ($deltas | Measure-Object -Maximum).Maximum }
    $largestNegative = if ($deltas.Count -eq 0) { $null } else { ($deltas | Measure-Object -Minimum).Minimum }
    $comparison = [ordered]@{
        schema_version = $script:BenchmarkComparisonSchemaVersion
        baseline = [ordered]@{ suite = $baselineData.Aggregate.suite }
        current = [ordered]@{ suite = $currentData.Aggregate.suite }
        summary = [ordered]@{
            compared_scenarios = $scenarios.Count
            largest_positive_delta_percent = $largestPositive
            largest_negative_delta_percent = $largestNegative
        }
        scenarios = $scenarios
    }
    $json = $comparison | ConvertTo-Json -Depth 100
    $parent = Split-Path -Parent $OutputPath
    if ($parent) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
    [System.IO.File]::WriteAllText($OutputPath, $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
    return $OutputPath
}
