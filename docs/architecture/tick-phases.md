# Canonical tick phases

These names are the stable profiling and benchmark contract for a Nexus tick.
They describe domain responsibilities rather than the current function or file
layout. A phase may occur in several blocks; its observations accumulate into
one value without changing execution order.

| Phase | Responsibility |
| --- | --- |
| `WorldMaintenance` | Renewable resource maintenance and recording mutations to world resources |
| `Physiology` | Hourly hunger, health, age, and other body-state advancement |
| `DependentCare` | Caregiver maintenance, infant colocation, and feeding dependents |
| `Households` | Migration planning and settlement, membership synchronization, dissolution, and inheritance |
| `SpatialIndex` | Rebuilding the population snapshot and spatial lookup used by autonomy |
| `Autonomy` | Perception, decision, planning, action execution, outcome integration, and social interaction |
| `Survival` | Resolving starvation damage after autonomous consumption |
| `Mortality` | Removing dead entities, processing grief, and reassigning orphaned dependents |
| `Lifecycle` | Pregnancy progression, births, and related biological maintenance |
| `Relationships` | Scheduled affinity decay and relationship maintenance |
| `Reproduction` | Scheduled conception attempts |

`total_us` measures the complete profiled tick, including the small orchestration
cost outside phase timers. Normal, phase-profiled, and autonomy-profiled modes
all execute these phases through the same canonical pipeline.
