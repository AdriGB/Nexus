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
