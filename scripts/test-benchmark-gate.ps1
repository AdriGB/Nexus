Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "benchmark-gate.ps1")

function Assert-Equal($Expected, $Actual, [string]$Message) { if ($Expected -cne $Actual) { throw "$Message Expected '$Expected', got '$Actual'." } }
function Assert-True($Value, [string]$Message) { if (-not $Value) { throw $Message } }
function Assert-Throws([scriptblock]$Action, [string]$Message) { try { & $Action | Out-Null } catch { return }; throw $Message }
function New-Scenario([string]$Name, $MeanDelta, [double]$BaselineMean = 1000, [double]$CurrentMean = 1100, [double]$PhaseDelta = 0) {
    [ordered]@{ name = $Name; timings = [ordered]@{
        total = [ordered]@{ mean = [ordered]@{ baseline_us = $BaselineMean; current_us = $CurrentMean; delta_percent = $MeanDelta } }
        autonomy = [ordered]@{ mean = [ordered]@{ delta_percent = $PhaseDelta } }
    }}
}
function Write-Comparison([string]$Path, [array]$Scenarios, [int]$Schema = 1) {
    [IO.File]::WriteAllText($Path, ([ordered]@{ schema_version = $Schema; scenarios = $Scenarios } | ConvertTo-Json -Depth 20), [Text.UTF8Encoding]::new($false))
}
function Write-RetryComparison([string]$Directory, [string]$Name, [double]$Delta) {
    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    Write-Comparison (Join-Path $Directory "benchmark-comparison.json") @((New-Scenario $Name $Delta))
}

$script:RetryInvocations = [System.Collections.Generic.List[string]]::new()
$script:RetryDeltas = @{}
function Reset-RetryScriptState { $script:RetryInvocations.Clear(); $script:RetryDeltas = @{} }
function Get-CountingRetryScript {
    return {
        param([string]$ScenarioName, [string]$OutputDir)
        $script:RetryInvocations.Add($ScenarioName)
        if ($script:RetryDeltas.ContainsKey($ScenarioName)) {
            Write-RetryComparison $OutputDir $ScenarioName $script:RetryDeltas[$ScenarioName]
        }
        elseif ($script:FailingRetry) {
            throw "runner exploded"
        }
    }
}

$root = Join-Path ([IO.Path]::GetTempPath()) "nexus-gate-$([guid]::NewGuid())"
New-Item -ItemType Directory $root | Out-Null
try {
    $comparisonPath = Join-Path $root "benchmark-comparison.json"

    # Candidate selection boundaries.
    Write-Comparison $comparisonPath @(
        (New-Scenario "below" 29.99), (New-Scenario "exact-thirty" 30.0), (New-Scenario "just-above" 30.01),
        (New-Scenario "micro-above" 30.000001), (New-Scenario "negative" -5), (New-Scenario "phase-spike" 5 1000 1050),
        (New-Scenario "null-delta" $null)
    )
    $scenarios = @(Get-BenchmarkGateCandidates -ComparisonPath $comparisonPath)
    Assert-Equal 2 $scenarios.Count "Candidate count differs."
    Assert-Equal "just-above" $scenarios[0].Name "Strictly greater than 30 must select candidates."
    Assert-Equal 30.01 $scenarios[0].FirstMeanDeltaPercent "Candidate delta was not preserved."
    Assert-Equal "micro-above" $scenarios[1].Name "30.000001 must be a candidate."
    Assert-True ((@($scenarios | Where-Object Name -in @("below", "exact-thirty", "negative", "phase-spike", "null-delta"))).Count -eq 0) "Non-candidates were selected."

    # A phase spike alone never selects a candidate.
    Write-Comparison $comparisonPath @((New-Scenario "autonomy-spike" 5 1000 1050 500))
    Assert-Equal 0 @(Get-BenchmarkGateCandidates -ComparisonPath $comparisonPath).Count "Phase spike was selected as candidate."

    # A comparison without scenarios selects nothing (micro suite has no aggregate).
    Write-Comparison $comparisonPath @()
    $empty = @(Get-BenchmarkGateCandidates -ComparisonPath $comparisonPath)
    Assert-Equal 0 $empty.Count "Empty comparison differs."

    # Recovered candidate: first >30, retry <=30 keeps the job green.
    Reset-RetryScriptState
    $script:FailingRetry = $false
    $script:RetryDeltas["alpha"] = 12.4
    Write-Comparison $comparisonPath @((New-Scenario "alpha" 35.2), (New-Scenario "normal" 5), (New-Scenario "negative" -20))
    $result = Invoke-BenchmarkGate -ComparisonPath $comparisonPath -RetryOutputDir (Join-Path $root "recovered") `
        -RetryScript (Get-CountingRetryScript)
    Assert-Equal 1 $result.Outcomes.Count "Only the candidate produced an outcome."
    Assert-Equal "Recovered" $result.Outcomes[0].Verdict "Recovered verdict differs."
    Assert-Equal 35.2 $result.Outcomes[0].FirstMeanDeltaPercent "First delta was not preserved."
    Assert-Equal 12.4 $result.Outcomes[0].RetryMeanDeltaPercent "Retry delta was not preserved."
    Assert-True (-not $result.HasConfirmedRegression) "Recovered must not confirm."
    Assert-True (-not $result.HasTechnicalFailure) "Recovered is not a technical failure."
    Assert-Equal 1 $script:RetryInvocations.Count "Exactly one confirmation run is allowed."

    # Confirmed candidates across magnitudes.
    foreach ($retryDelta in @(31.0, 41.0, 100.0)) {
        $directory = Join-Path $root "confirmed-$retryDelta"
        Reset-RetryScriptState
        $script:RetryDeltas["alpha"] = $retryDelta
        $result = Invoke-BenchmarkGate -ComparisonPath $comparisonPath -RetryOutputDir $directory `
            -RetryScript (Get-CountingRetryScript)
        Assert-Equal "Confirmed" $result.Outcomes[0].Verdict "Confirmed verdict differs for retry $retryDelta."
        Assert-True $result.HasConfirmedRegression "Confirmed regression flag differs for retry $retryDelta."
    }

    # An invalid confirmation run is a technical failure, never recovery.
    Reset-RetryScriptState
    $script:FailingRetry = $true
    $result = Invoke-BenchmarkGate -ComparisonPath $comparisonPath -RetryOutputDir (Join-Path $root "technical") `
        -RetryScript (Get-CountingRetryScript)
    Assert-Equal "Technical failure" $result.Outcomes[0].Verdict "Invalid retry must be a technical failure."
    Assert-True ($null -eq $result.Outcomes[0].RetryMeanDeltaPercent) "Technical failure has no retry delta."
    Assert-True $result.HasTechnicalFailure "Technical failure flag differs."
    Assert-True (-not $result.HasConfirmedRegression) "A broken run must not count as recovered."
    Assert-Equal 1 $script:RetryInvocations.Count "Broken retries still run exactly once."

    # A retry comparison without the retried scenario is also technical.
    $missing = Join-Path $root "missing-scenario"
    New-Item -ItemType Directory -Force -Path $missing | Out-Null
    Write-Comparison (Join-Path $missing "benchmark-comparison.json") @((New-Scenario "other" 10))
    $result = Invoke-BenchmarkGate -ComparisonPath $comparisonPath -RetryOutputDir $missing `
        -RetryScript { param([string]$ScenarioName, [string]$OutputDir) }
    Assert-Equal "Technical failure" $result.Outcomes[0].Verdict "Missing retry scenario must be technical."

    # Multiple candidates repeat independently; non-candidates never repeat.
    Reset-RetryScriptState
    $script:FailingRetry = $false
    $script:RetryDeltas["alpha"] = 12.4
    $script:RetryDeltas["beta"] = 44.8
    Write-Comparison $comparisonPath @((New-Scenario "beta" 42.1), (New-Scenario "alpha" 35.2), (New-Scenario "gamma" 8))
    $result = Invoke-BenchmarkGate -ComparisonPath $comparisonPath -RetryOutputDir (Join-Path $root "multi") `
        -RetryScript (Get-CountingRetryScript)
    Assert-Equal 2 $result.Outcomes.Count "Outcome count differs."
    Assert-Equal "alpha" $result.Outcomes[0].Name "Outcomes must stay ordered by scenario name."
    Assert-Equal "beta" $result.Outcomes[1].Name "Outcomes must stay ordered by scenario name."
    Assert-Equal "Recovered" $result.Outcomes[0].Verdict "Alpha outcome differs."
    Assert-Equal "Confirmed" $result.Outcomes[1].Verdict "Beta outcome differs."
    Assert-True $result.HasConfirmedRegression "Mixed outcomes must fail."
    Assert-Equal 2 $script:RetryInvocations.Count "Each candidate repeats exactly once."
    Assert-True (-not $script:RetryInvocations.Contains("gamma")) "Non-candidates must never repeat."
    $markdown = Get-BenchmarkGateMarkdown -Outcomes $result.Outcomes
    Assert-True ($markdown.IndexOf("| alpha | +35.20% | +12.40% | Recovered |") -lt $markdown.IndexOf("| beta | +42.10% | +44.80% | Confirmed |")) "Summary rows differ or lost stable order."
    Assert-True ($markdown -notmatch "gamma") "Summary must not mention non-candidates."

    # Summary wording distinguishes empty, recovered, and confirmed states.
    Assert-True ((Get-BenchmarkGateMarkdown -Outcomes @()) -match "No >30% candidate regressions detected\.") "Empty summary wording differs."
    $recoveredOutcome = [pscustomobject]@{ Name = "alpha"; FirstMeanDeltaPercent = 35.2; RetryMeanDeltaPercent = 12.4; Verdict = "Recovered"; Detail = $null }
    $recoveredSummary = Get-BenchmarkGateMarkdown -Outcomes @($recoveredOutcome)
    Assert-True ($recoveredSummary -match "No confirmed performance regressions\.") "Recovered summary wording differs."
    $confirmedOutcome = [pscustomobject]@{ Name = "beta"; FirstMeanDeltaPercent = 42.1; RetryMeanDeltaPercent = 44.8; Verdict = "Confirmed"; Detail = $null }
    $confirmedSummary = Get-BenchmarkGateMarkdown -Outcomes @($confirmedOutcome)
    Assert-True ($confirmedSummary -notmatch "No confirmed performance regressions\.") "Confirmed summary wording differs."
    Assert-True ((Get-BenchmarkGateMarkdown -Outcomes @()) -match "reproduction on a second run") "Noise note missing."

    # Annotations: one candidate announcement, then one outcome annotation per scenario.
    $candidateAnnouncements = @(Write-GitHubBenchmarkCandidateAnnouncements -Candidates @(
        [pscustomobject]@{ Name = "alpha"; FirstMeanDeltaPercent = 35.2 },
        [pscustomobject]@{ Name = "beta"; FirstMeanDeltaPercent = 42.1 }
    ))
    Assert-Equal 2 $candidateAnnouncements.Count "Candidate announcements differ."
    Assert-True ($candidateAnnouncements[0] -match '^::warning title=Performance candidate::alpha%3A total mean \+35\.20%') "Candidate announcement format differs."
    $outcomeAnnotations = @(Write-GitHubBenchmarkGateOutcomeAnnotations -Outcomes @($recoveredOutcome, $confirmedOutcome))
    Assert-Equal 2 $outcomeAnnotations.Count "Outcome annotations differ."
    Assert-True ($outcomeAnnotations[0] -match '^::warning title=Performance candidate not reproduced::') "Recovered stays a warning."
    Assert-True ($outcomeAnnotations[1] -match '^::error title=Confirmed performance regression::Confirmed performance regression%3A beta total mean \+44\.80%25\.') "Confirmed annotation format differs."

    # Attribution is diagnostic: it never selects a candidate or changes the verdict.
    $attributed = [ordered]@{
        name = "explained"
        timings = [ordered]@{
            total = [ordered]@{ mean = [ordered]@{ baseline_us = 1000; current_us = 1400; delta_percent = 40 } }
            autonomy = [ordered]@{ mean = [ordered]@{ baseline_us = 800; current_us = 1100; delta_percent = 37.5 } }
            households = [ordered]@{ mean = [ordered]@{ baseline_us = 1; current_us = 5; delta_percent = 400 } }
        }
        work = [ordered]@{
            pathfinding_nodes_expanded = [ordered]@{ baseline = 10000; current = 22000; delta_percent = 120 }
            actions_executed = [ordered]@{ baseline = 100; current = 90; delta_percent = -10 }
        }
        state_peak = [ordered]@{
            known_entities_total = [ordered]@{ baseline = 100; current = 150; delta_percent = 50 }
        }
    }
    $workSpike = [ordered]@{
        name = "work-spike"
        timings = [ordered]@{
            total = [ordered]@{ mean = [ordered]@{ baseline_us = 1000; current_us = 1050; delta_percent = 5 } }
        }
        work = [ordered]@{
            pathfinding_nodes_expanded = [ordered]@{ baseline = 100; current = 100000; delta_percent = 99900 }
        }
    }
    Write-Comparison $comparisonPath @($attributed, $workSpike)
    $attributedCandidates = @(Get-BenchmarkGateCandidates -ComparisonPath $comparisonPath)
    Assert-Equal 1 $attributedCandidates.Count "Work-counter spikes must not create candidates."
    Assert-Equal "explained" $attributedCandidates[0].Name "Attributed candidate differs."
    Assert-True ($attributedCandidates[0].Explanation -match "autonomy \+300\.00 us") "Candidate explanation must rank absolute microseconds."
    Assert-True ($attributedCandidates[0].Explanation -match "pathfinding_nodes_expanded \+12000") "Candidate explanation must name the dominant counter."
    Assert-True ($attributedCandidates[0].Explanation -notmatch "actions_executed") "Decreased counters must be omitted."

    Reset-RetryScriptState
    $script:FailingRetry = $false
    $script:RetryDeltas["explained"] = 41.0
    $result = Invoke-BenchmarkGate -ComparisonPath $comparisonPath -RetryOutputDir (Join-Path $root "attributed-retry") `
        -RetryScript (Get-CountingRetryScript)
    Assert-Equal 1 $result.Candidates.Count "Gate must still retry only total.mean candidates."
    Assert-Equal "Confirmed" $result.Outcomes[0].Verdict "Attributed candidate must still gate only on total.mean."
    Assert-True ($result.Outcomes[0].Explanation -match "autonomy \+300\.00 us") "Confirmed outcome must keep first-run attribution when retry has none."
    $gateMarkdown = Get-BenchmarkGateMarkdown -Outcomes $result.Outcomes
    Assert-True ($gateMarkdown -match "Explained by") "Gate markdown must include attribution."
    Assert-True ($gateMarkdown -match "autonomy \+300\.00 us") "Gate markdown attribution differs."
    $attributedAnnouncements = @(Write-GitHubBenchmarkCandidateAnnouncements -Candidates $attributedCandidates)
    Assert-True ($attributedAnnouncements[0] -match "autonomy \+300\.00 us") "Candidate announcements must include attribution."
    $confirmedAnnotations = @(Write-GitHubBenchmarkGateOutcomeAnnotations -Outcomes $result.Outcomes)
    Assert-True ($confirmedAnnotations[0] -match "autonomy \+300\.00 us") "Confirmed annotations must include attribution."

    # Invalid comparisons are rejected instead of silently passing.
    Write-Comparison $comparisonPath @((New-Scenario "bad-schema" 31)) 2
    Assert-Throws { Get-BenchmarkGateCandidates -ComparisonPath $comparisonPath } "Invalid comparison schema was accepted."
    $malformed = New-Scenario "malformed" 31; $malformed.timings.total = $null
    Write-Comparison $comparisonPath @($malformed)
    Assert-Throws { Get-BenchmarkGateCandidates -ComparisonPath $comparisonPath } "Malformed timings were accepted."
    Assert-Throws { Get-BenchmarkGateCandidates -ComparisonPath (Join-Path $root "absent.json") } "Missing comparison was accepted."

    Write-Host "Benchmark gate tests passed." -ForegroundColor Green
}
finally {
    $resolved = [IO.Path]::GetFullPath($root)
    if ($resolved.StartsWith([IO.Path]::GetFullPath([IO.Path]::GetTempPath()), [StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}
