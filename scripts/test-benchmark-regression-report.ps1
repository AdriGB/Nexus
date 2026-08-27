Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-regression-report.ps1")

function Assert-Equal($Expected, $Actual, [string]$Message) { if ($Expected -cne $Actual) { throw "$Message Expected '$Expected', got '$Actual'." } }
function Assert-True($Value, [string]$Message) { if (-not $Value) { throw $Message } }
function Assert-Throws([scriptblock]$Action, [string]$Message) { try { & $Action | Out-Null } catch { return }; throw $Message }
function New-Scenario([string]$Name, $MeanDelta, [double]$BaselineMean = 1000, [double]$CurrentMean = 1100, [double]$P95Delta = 4, [double]$PhaseDelta = 0) {
    [ordered]@{ name = $Name; timings = [ordered]@{
        total = [ordered]@{ mean = [ordered]@{ baseline_us = $BaselineMean; current_us = $CurrentMean; delta_percent = $MeanDelta }; p95 = [ordered]@{ baseline_us = 1200; current_us = 1248; delta_percent = $P95Delta } }
        households = [ordered]@{ mean = [ordered]@{ delta_percent = $PhaseDelta } }
    }}
}
function New-PhaseMean([double]$Baseline, [double]$Current) {
    [ordered]@{ mean = [ordered]@{ baseline_us = $Baseline; current_us = $Current; delta_percent = if ($Baseline -eq 0) { if ($Current -eq 0) { 0 } else { $null } } else { [math]::Round((($Current - $Baseline) / $Baseline) * 100.0, 6) } } }
}
function New-Count([double]$Baseline, [double]$Current) {
    [ordered]@{ baseline = $Baseline; current = $Current; delta_percent = if ($Baseline -eq 0) { if ($Current -eq 0) { 0 } else { $null } } else { [math]::Round((($Current - $Baseline) / $Baseline) * 100.0, 6) } }
}
function New-AttributedScenario {
    param(
        [string]$Name,
        [double]$MeanDelta = 25,
        [double]$BaselineMean = 1000,
        [double]$CurrentMean = 1250,
        [double]$AutonomyBaseline = 800,
        [double]$AutonomyCurrent = 1000,
        [double]$HouseholdsBaseline = 1,
        [double]$HouseholdsCurrent = 5,
        [double]$NodesBaseline = 10000,
        [double]$NodesCurrent = 22000,
        [double]$GoalBaseline = 10,
        [double]$GoalCurrent = 11,
        [double]$KnownBaseline = 100,
        [double]$KnownCurrent = 150
    )
    [ordered]@{
        name = $Name
        timings = [ordered]@{
            total = [ordered]@{
                mean = [ordered]@{ baseline_us = $BaselineMean; current_us = $CurrentMean; delta_percent = $MeanDelta }
                p95 = [ordered]@{ baseline_us = 1200; current_us = 1248; delta_percent = 4 }
            }
            autonomy = New-PhaseMean $AutonomyBaseline $AutonomyCurrent
            households = New-PhaseMean $HouseholdsBaseline $HouseholdsCurrent
            physiology = New-PhaseMean 10 10
        }
        work = [ordered]@{
            pathfinding_nodes_expanded = New-Count $NodesBaseline $NodesCurrent
            goal_changes = New-Count $GoalBaseline $GoalCurrent
            actions_executed = New-Count 100 90
        }
        state_peak = [ordered]@{
            known_entities_total = New-Count $KnownBaseline $KnownCurrent
            entities_alive = New-Count 100 100
        }
    }
}
function Write-Comparison([string]$Path, [array]$Scenarios, [int]$Schema = 1) { [IO.File]::WriteAllText($Path, ([ordered]@{ schema_version = $Schema; scenarios = $Scenarios } | ConvertTo-Json -Depth 20), [Text.UTF8Encoding]::new($false)) }

$root = Join-Path ([IO.Path]::GetTempPath()) "nexus-report-$([guid]::NewGuid())"
New-Item -ItemType Directory $root | Out-Null
try {
    $path = Join-Path $root "comparison.json"
    Write-Comparison $path @(
        (New-Scenario "below" 9.99), (New-Scenario "ten" 10.0), (New-Scenario "info-low" 10.01), (New-Scenario "info-high" 20.0),
        (New-Scenario "warning-low" 20.01 1000 1200.1 7), (New-Scenario "warning-large" 100), (New-Scenario "negative" -20),
        (New-Scenario "unavailable" $null), (New-Scenario "phase-noise" 5 1000 1050 3 500), (New-Scenario "total-info" 15 1000 1150 2 1),
        (New-Scenario "total-warning" 25 1000 1250 2 1), (New-Scenario "warning-thirty" 30)
    )
    $items = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal 7 $items.Count "Observation count differs."
    Assert-Equal "warning-large" $items[0].Name "Warnings must sort first by delta."
    Assert-Equal "warning-thirty" $items[1].Name "Warnings must sort before info."
    Assert-Equal "total-warning" $items[2].Name "Warning ordering differs."
    Assert-Equal "warning-low" $items[3].Name "Warning boundary differs."
    Assert-Equal "Warning" $items[3].Level "20.01 must be warning."
    Assert-Equal "Informational" $items[4].Level "20.00 must remain informational."
    Assert-Equal "total-info" $items[5].Name "Informational ordering differs."
    Assert-Equal 1000 $items[3].BaselineMeanUs "Baseline mean was not preserved."
    Assert-Equal 1200.1 $items[3].CurrentMeanUs "Current mean was not preserved."
    Assert-Equal 7 $items[3].P95DeltaPercent "p95 context was not preserved."
    Assert-Equal "" $items[0].Explanation "Scenarios without counters must still report with empty attribution."

    Write-Comparison $path @((New-Scenario "zeta" 25), (New-Scenario "alpha" 25), (New-Scenario "info-z" 15), (New-Scenario "info-a" 15))
    $ties = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal "alpha" $ties[0].Name "Warning tie order differs."
    Assert-Equal "zeta" $ties[1].Name "Warning tie order differs."
    Assert-Equal "info-a" $ties[2].Name "Info tie order differs."
    $markdown = Get-InformationalBenchmarkMarkdown $ties
    if ($markdown.IndexOf("| Warning | alpha") -gt $markdown.IndexOf("| Info | info-a")) { throw "Markdown does not put warnings first." }
    $annotations = @(Write-GitHubBenchmarkWarningAnnotations $ties)
    Assert-Equal 2 $annotations.Count "Warnings must emit one annotation each."
    if ($annotations[0] -notmatch '^::warning title=Potential performance slowdown::alpha%3A total mean \+25\.00%') { throw "Warning annotation is malformed." }
    if ($annotations -match "info-a") { throw "Informational scenarios emitted warnings." }

    # Gate candidates above the exclusion threshold are announced by the gate instead.
    $withCandidate = @(
        (New-Scenario "generic-only" 25),
        (New-Scenario "gate-candidate" 35.2),
        (New-Scenario "boundary-thirty" 30),
        (New-Scenario "info" 15)
    )
    Write-Comparison $path $withCandidate
    $mixed = @(Get-BenchmarkPerformanceObservations $path)
    $deduplicated = @(Write-GitHubBenchmarkWarningAnnotations -Observations $mixed -ExcludeCandidatesAbovePercent 30)
    Assert-Equal 2 $deduplicated.Count "Only non-candidate warnings must remain when candidates are excluded."
    Assert-True ($deduplicated[0] -match '^::warning title=Potential performance slowdown::boundary-thirty') "Exclusion kept the wrong warning."
    Assert-True ($deduplicated[1] -match '^::warning title=Potential performance slowdown::generic-only') "Exclusion kept the wrong warning."
    Assert-True ((@($deduplicated | Where-Object { $_ -match "gate-candidate" })).Count -eq 0) "Candidate warning was not suppressed."
    Assert-Equal 3 @(Write-GitHubBenchmarkWarningAnnotations -Observations $mixed).Count "Without exclusion every warning must be emitted."

    Write-Comparison $path @((New-Scenario "none" 10))
    $none = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal 0 $none.Count "Zero-report case differs."
    if ((Get-InformationalBenchmarkMarkdown $none) -notmatch "No performance slowdowns") { throw "Zero-report Markdown is unclear." }

    Write-Comparison $path @((New-Scenario "bad-schema" 11)) 2
    Assert-Throws { Get-BenchmarkPerformanceObservations $path } "Invalid comparison schema was accepted."
    $malformed = New-Scenario "malformed" 11; $malformed.timings.total.p95 = $null
    Write-Comparison $path @($malformed)
    Assert-Throws { Get-BenchmarkPerformanceObservations $path } "Malformed timing was accepted."

    # Attribution ranks absolute microseconds, not a tiny phase's percentage spike.
    Write-Comparison $path @(
        (New-AttributedScenario "explained"),
        (New-AttributedScenario "phase-only-noise" -MeanDelta 5 -BaselineMean 1000 -CurrentMean 1050)
    )
    $attributed = @(Get-BenchmarkPerformanceObservations $path)
    Assert-Equal 1 $attributed.Count "A phase spike without a total-mean slowdown must not be reported."
    Assert-Equal "explained" $attributed[0].Name "Attributed scenario differs."
    Assert-Equal 2 @($attributed[0].Attribution.Phases).Count "Phase attribution count differs."
    Assert-Equal "autonomy" $attributed[0].Attribution.Phases[0].Name "Absolute microseconds must outrank a tiny high-percent phase."
    Assert-Equal 200 $attributed[0].Attribution.Phases[0].AbsoluteDelta "Autonomy absolute delta differs."
    Assert-Equal "households" $attributed[0].Attribution.Phases[1].Name "Secondary phase order differs."
    Assert-Equal "pathfinding_nodes_expanded" $attributed[0].Attribution.Work[0].Name "Work attribution must rank absolute count increases."
    Assert-Equal 12000 $attributed[0].Attribution.Work[0].AbsoluteDelta "Work absolute delta differs."
    Assert-Equal "goal_changes" $attributed[0].Attribution.Work[1].Name "Secondary work order differs."
    Assert-Equal 1 @($attributed[0].Attribution.State).Count "Single grown state gauge must not unwrap."
    Assert-Equal "known_entities_total" $attributed[0].Attribution.State[0].Name "Grown state gauges must appear."
    Assert-Equal 0 @($attributed[0].Attribution.State | Where-Object Name -eq "entities_alive").Count "Unchanged state gauges must be omitted."
    Assert-True ($attributed[0].Explanation -match "autonomy \+200\.00 us") "Explanation must name the dominant phase."
    Assert-True ($attributed[0].Explanation -match "pathfinding_nodes_expanded \+12000") "Explanation must name the dominant counter."
    Assert-True ($attributed[0].Explanation -notmatch "physiology") "Unchanged phases must be omitted."
    Assert-True ($attributed[0].Explanation -notmatch "actions_executed") "Decreased counters must be omitted."
    $explainedMarkdown = Get-InformationalBenchmarkMarkdown $attributed
    Assert-True ($explainedMarkdown -match "Explained by") "Markdown must include attribution."
    Assert-True ($explainedMarkdown -match "autonomy \+200\.00 us") "Markdown attribution differs."
    $explainedAnnotations = @(Write-GitHubBenchmarkWarningAnnotations $attributed)
    Assert-True ($explainedAnnotations[0] -match "autonomy \+200\.00 us") "Warning annotations must include attribution."

    # Attribution limit is stable and uses ordinal name ties after absolute delta.
    $tied = New-AttributedScenario "ties" -AutonomyBaseline 100 -AutonomyCurrent 150 -HouseholdsBaseline 10 -HouseholdsCurrent 60
    $tied.timings.mortality = New-PhaseMean 20 70
    $tied.timings.lifecycle = New-PhaseMean 0 1
    $attribution = Get-BenchmarkRegressionAttribution -Scenario $tied
    Assert-Equal 3 @($attribution.Phases).Count "Attribution must cap phases."
    Assert-Equal "autonomy" $attribution.Phases[0].Name "Tied absolute deltas must keep ordinal order."
    Assert-Equal "households" $attribution.Phases[1].Name "Tied absolute deltas must keep ordinal order."
    Assert-Equal "mortality" $attribution.Phases[2].Name "Tied absolute deltas must keep ordinal order."
    Assert-Equal 0 @($attribution.Phases | Where-Object Name -eq "lifecycle").Count "The fourth phase must be dropped by the cap."

    Write-Host "Benchmark regression report tests passed." -ForegroundColor Green
}
finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase)) { Remove-Item -LiteralPath $resolved -Recurse -Force }
}
