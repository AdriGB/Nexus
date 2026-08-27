# Aggregate benchmark results

`benchmark-results.json` is the stable aggregate contract for end-to-end Nexus
benchmark scenarios. Aggregate schema v1 has this shape:

```json
{
  "schema_version": 1,
  "suite": "quick",
  "results": [
    { "schema_version": 3, "scenario": { "name": "baseline-100" } },
    { "schema_version": 3, "scenario": { "name": "baseline-1000" } }
  ]
}
```

Each entry in `results` is the complete validated payload emitted by
`nexus-bench`; the aggregate does not reinterpret timings or discard long-run
windows. Entries are ordered by scenario name. The contract intentionally omits
timestamps, host details, paths, commit IDs, and other volatile provenance.

The `quick`, `scenarios`, and `full` suites create the aggregate, as does a
single-scenario execution. The Criterion-only `micro` suite does not. Raw
scenario files remain diagnostic outputs, while `benchmark-results.json` is the
canonical input for baseline comparison and the performance gate.

## Baseline and comparison

The reviewed reference measurement lives at
`benchmarks/baselines/github-ubuntu-x64/benchmark-results.json`. It is a Full
GitHub-hosted Ubuntu measurement and is changed only through a deliberate PR.
`scripts/update-benchmark-baseline.ps1` validates and copies a downloaded Full
aggregate; it never runs benchmarks or downloads artifacts.

Runs containing scenarios compare their current aggregate with that Full
baseline and write `benchmark-comparison.json` schema v1. Each current scenario
must have identical deterministic scenario metadata in the baseline. The output
contains baseline/current values and `((current - baseline) / baseline) * 100`
for total and every canonical phase across mean, median, p95, p99, and max,
plus the same percent delta for each `work_total` counter and `state_peak`
gauge. Positive timing values are slower and negative values are faster. A
nonzero current value over a zero baseline has a `null` delta. Long-run
comparisons use the complete input but currently compare its `overall` summary,
not each window.

The comparison itself is mathematical. Classification, GitHub annotations, and
the performance gate are separate reporting layers that consume it.

## Informational slowdown report

The reporting layer selects a scenario only when its
`timings.total.mean.delta_percent` is strictly greater than 10%. Canonical phase
spikes and other total statistics never activate the report; total p95 appears
only as context. The 10% level is informational only and may include runner
noise.

Calibration during PR #160 illustrates that noise: three GitHub-hosted Quick
runs measured `baseline-1000` total mean at 33,686.65 us (+14.189944%),
30,422.18 us (+3.124146%), and 34,560.62 us (+17.152500%) against the same
29,500.54 us baseline. Therefore the informational level changes neither exit
codes nor job status.

## Significant slowdown warnings

The same `timings.total.mean.delta_percent` signal becomes a GitHub Actions
warning only when it is strictly greater than 20%. Values from 10% through 20%
remain informational. A warning means a potential slowdown worth attention, not
a confirmed regression: GitHub-hosted runner noise can still contribute.

Warnings are emitted once per scenario in Actions and include total mean delta,
baseline mean, and current mean. Scenarios already announced as performance
gate candidates are excluded from this generic warning so each scenario
receives at most one pre-retry annotation. Local runs print ordinary `WARNING:`
text instead. Neither form changes the benchmark exit code.

## Performance gate

The gate is the blocking layer of the policy ladder:

| Total mean delta | Classification | Job effect |
| --- | --- | --- |
| <= 10% | None | Success |
| > 10%, <= 20% | Informational | Success |
| > 20%, <= 30% | Warning | Success |
| > 30% | Candidate regression | Pending confirmation |
| Candidate reproduced above 30% | Confirmed regression | Failure |
| Candidate not reproduced above 30% | Recovered | Success |

The signal is strictly `scenario.timings.total.mean.delta_percent`. Exactly
30.000000% is not a candidate; 30.000001% is. Phase timings, p95, p99, max,
work counters, state gauges, and the comparison summary never activate or
soften the gate; they remain diagnostic context.

A candidate regression must be reproduced before it blocks. The gate re-runs
only the candidate scenario — through the ordinary single-scenario benchmark
entry point with identical scenario metadata (seed, population, warmup,
measured ticks, workload) — into an isolated output directory
(`target/nexus-bench/retry/`, including its own aggregate and comparison files)
and compares that second measurement against the same Full baseline. Nothing
else is re-run: Quick retries one candidate scenario, nightly Full retries for
example only `long-run-1000`. Each candidate receives exactly one confirmation
run; there are no retry loops, medians of N, or other aggregation.

Outcomes:

- **Recovered** — the retry measures at or below 30%. The job stays green, a
  warning annotation notes that the candidate was not reproduced, and the
  summary row reads `Recovered`.
- **Confirmed** — the retry also measures strictly above 30%. The job fails,
  one error annotation names the confirmed scenario, and the summary row reads
  `Confirmed`.
- **Technical failure** — the confirmation run itself breaks (build, execution,
  invalid output). This is a broken benchmark, not a recovery: the job fails.

Performance differences alone therefore produce a non-zero exit code only for
confirmed regressions or technical failures; warnings never block. The gate is
skipped for the Criterion-only `micro` suite because it has no aggregate or
comparison. Retry outputs are preserved inside the raw results artifact for
auditability: the gate runs before artifact upload, so `retry/` is included
whenever a confirmation run happens. Baselines are never updated
automatically: an intentional slowdown is absorbed later by an explicit
baseline-update PR.

One confirmation run is required before blocking because shared GitHub-hosted
runners are noisy. Reproduction on a second run is an operational confirmation
policy, not a statistical guarantee or proof.

## Regression attribution

When a scenario is reported — informational, warning, candidate, or confirmed —
the report names the canonical phases and the work/state counters that explain
the `total.mean` increase. Attribution ranks absolute increases, not
percentages: a 4 µs phase that jumped 400% does not outrank a 200 µs autonomy
increase. Unchanged or decreased values are omitted. At most three items of
each kind are shown, with ordinal name as the tie-break.

A phase or counter spike without a `total.mean` slowdown never creates an
observation, annotation, or candidate. The gate still blocks only on a
confirmed `total.mean` retry. Attribution appears in local output, GitHub
annotations, and the job summary as an `Explained by` column.
