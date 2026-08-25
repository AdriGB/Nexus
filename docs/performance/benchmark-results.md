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
canonical input for future baseline comparison. No baseline, regression
comparison, threshold, or performance gate exists yet.

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
for total and every canonical phase across mean, median, p95, p99, and max.
Positive values are slower and negative values are faster. A nonzero current
value over a zero baseline has a `null` delta. Long-run comparisons use the
complete input but currently compare its `overall` summary, not each window.

The comparison is mathematical only: there are no thresholds, classifications,
warnings, or performance gates.

## Informational slowdown report

The reporting layer selects a scenario only when its
`timings.total.mean.delta_percent` is strictly greater than 10%. Canonical phase
spikes and other total statistics never activate the report; total p95 appears
only as context. The 10% level is informational only and may include runner
noise.

Calibration during PR #160 illustrates that noise: three GitHub-hosted Quick
runs measured `baseline-1000` total mean at 33,686.65 us (+14.189944%),
30,422.18 us (+3.124146%), and 34,560.62 us (+17.152500%) against the same
29,500.54 us baseline. Therefore the report neither changes exit codes nor uses
GitHub warning annotations. The future 20% and 30% policies remain separate.

## Significant slowdown warnings

The same `timings.total.mean.delta_percent` signal becomes a GitHub Actions
warning only when it is strictly greater than 20%. Values from 10% through 20%
remain informational. A warning means a potential slowdown worth attention, not
a confirmed regression: GitHub-hosted runner noise can still contribute.

Warnings are emitted once per scenario in Actions and include total mean delta,
baseline mean, and current mean. Local runs print ordinary `WARNING:` text
instead. Neither form changes the benchmark exit code; the future 30% gate is a
separate policy.
