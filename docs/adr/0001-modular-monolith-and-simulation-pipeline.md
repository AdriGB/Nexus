# ADR 0001: Modular monolith and explicit simulation pipeline

- Status: accepted
- Date: 2026-08-18

## Context

Nexus will add inventories, households, rules, settlements, economy, culture,
and history. Keeping their orchestration and implementation inside
`Simulation` would make unrelated features change the same file and encourage
direct coupling between domains.

The engine is a deterministic Rust/WASM application. Splitting it into services
would add operational and consistency costs without solving the current source
code boundary problem.

## Decision

Nexus remains a modular monolith.

The simulation pipeline owns the ordered lifecycle of a tick. `Simulation`
owns state and provides operations used by that pipeline. Domain modules own
their rules and invariants. The WASM bridge translates data at the boundary and
does not contain domain behavior.

Dependencies follow the direction documented in `docs/architecture/README.md`.
Module internals remain private unless a concrete cross-module contract is
required.

## Consequences

- The complete tick order has a single, discoverable home.
- New domains can be added without expanding the public surface of every
  existing domain.
- Pipeline changes and domain-rule changes can be reviewed separately.
- Some methods remain on `Simulation` during incremental extraction. They may
  move behind domain APIs later without requiring a big-bang rewrite.
- Profiled and normal execution paths must remain behaviorally equivalent and
  are protected by parity tests.

## Alternatives considered

- Keep orchestration in `Simulation`: simple initially, but concentrates future
  changes in an already large module.
- Split the engine into microservices: adds deployment and determinism problems
  with no present operational need.
- Introduce a generic plugin/ECS framework immediately: too speculative before
  the next concrete domains establish shared requirements.
