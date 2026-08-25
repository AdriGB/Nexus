# Nexus architecture

Nexus is a modular monolith. Features live in one deployable engine, but each
domain owns its rules and exposes a deliberately small API.

## Dependency direction

Dependencies point inward:

```text
web UI -> WASM bridge -> simulation orchestration -> domain modules
                             |                       |
                             +---- infrastructure ---+
```

- Domain modules must not depend on the web UI, WASM DTOs, or rendering.
- The bridge translates engine queries into transport types; it does not own
  simulation rules.
- The simulation pipeline defines execution order; domain modules define what
  each operation means.
- Infrastructure such as pathfinding and spatial indexes supports domains but
  must not become the owner of domain policy.
- New cross-domain behavior should use explicit commands, queries, or events
  instead of reaching into another module's internal representation.

## Module contract

Every growing domain should document:

1. the state and invariants it owns;
2. the public commands and queries it accepts;
3. the events it produces or consumes;
4. its allowed dependencies;
5. unit tests for its rules and integration tests for cross-domain behavior.

Keep module internals private by default. Promote an item to `pub(crate)` only
when another engine module has a concrete need for the contract.

The WASM bridge is grouped by transport responsibility:

```text
bridge/
├── entities.rs  # entity and relationship views
├── events.rs    # event history views
├── profiles.rs  # simulation statistics and diagnostics
└── world.rs     # tile and region views
```

`bridge.rs` is the facade. Consumers depend on the facade exports rather than
on DTO modules directly, so transport internals can evolve independently.

## Change shape

Prefer small vertical changes that leave `main` usable. A feature may touch a
domain, pipeline, event contract, bridge, and UI, but each layer should contain
only its own responsibility. Avoid adding generic extension points until at
least two concrete domains need the same abstraction.

Architectural decisions that constrain future work belong in `docs/adr`.

The Phase 2.11 baseline, including the current dependency map, state ownership,
invariants, and incremental extraction order, is recorded in
[`2.11-audit.md`](2.11-audit.md).
