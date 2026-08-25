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
