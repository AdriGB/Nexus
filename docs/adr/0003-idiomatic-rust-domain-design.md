# ADR 0003: Idiomatic Rust domain design

- Status: accepted
- Date: 2026-08-18

## Context

SOLID helps define module boundaries, but Nexus also needs conventions that use
Rust's ownership model and type system directly. Applying Java-style dependency
injection or typestate to every dynamic entity would add ceremony and could
make heterogeneous simulation state harder to represent.

## Decision

Nexus uses the following Rust design rules:

- Function signatures request the least authority required: `&T` for shared
  reads, `&mut T` for mutation, and `T` only when ownership is consumed.
- Newtypes distinguish identifiers, ticks, quantities, and validated values
  when confusing their primitive representations would be plausible.
- `enum`, `Option`, and `Result` represent closed alternatives and expected
  absence or failure; sentinel values remain private implementation details.
- Validation happens at external boundaries and constructors preserve domain
  invariants afterward.
- Typestate is reserved for stable, linear protocols such as validated world
  construction or save migration. Dynamic entity state uses enums and data.
- Concrete types and enums are the default. Generics or traits are introduced
  when they reduce assumptions or support real substitution.
- Static dispatch is preferred for closed, performance-sensitive algorithms;
  `dyn Trait` remains valid for runtime-selected heterogeneous behavior.
- `Send` and `Sync` are necessary but not sufficient for parallel simulation;
  deterministic ordering and reduction must also be designed explicitly.

The Rust API Guidelines are treated as recommendations, especially for the
crate boundary, rather than as mandatory rules for every internal function.

## First application

`SimulationEvent::id` uses the `EventId` newtype instead of a raw `u64`.
`PendingSimulationEvent` represents an event before history assignment, so a
registered event with an unassigned ID cannot be represented. Incrementing an
ID is owned by the type rather than repeated as primitive arithmetic.

`EntityId` and `Tick` will be migrated separately because they cross most of
the engine. Keeping those changes isolated makes their contracts reviewable and
avoids combining mechanical churn with behavioral features.

## Consequences

- Some boundary code must convert domain values to transport primitives.
- Invalid cross-assignment between event IDs and ticks becomes a compile error.
- Newtypes add small amounts of explicit conversion but no runtime overhead.
- Broad type migrations happen incrementally and receive focused tests.
