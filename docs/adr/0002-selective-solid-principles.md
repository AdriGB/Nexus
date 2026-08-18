# ADR 0002: Apply SOLID selectively

- Status: accepted
- Date: 2026-08-18

## Context

Nexus needs source-code boundaries that remain understandable as inventories,
households, rules, settlements, and culture are added. SOLID offers useful
guidance, but applying object-oriented patterns literally in Rust would create
premature traits, indirection, and extension points without real consumers.

## Decision

Nexus applies SOLID according to these priorities:

1. **Single responsibility:** modules own one cohesive reason to change.
2. **Interface segregation:** APIs and traits expose the smallest capability a
   consumer needs.
3. **Dependency inversion:** domain rules do not depend on WASM, JSON, UI, or
   rendering details.
4. **Open/closed:** extension points are extracted only after two concrete use
   cases demonstrate a stable shared contract.
5. **Liskov substitution:** interchangeable trait implementations must preserve
   determinism, invariants, and documented side effects.

Rust modules, visibility, enums, composition, and pure functions are preferred
over inheritance-style abstractions. Traits require a concrete substitution,
isolation, or multi-implementation need; they are not introduced solely to
wrap a single implementation.

## Enforcement

- New modules keep internals private by default.
- Domain operations accept only the state they need rather than the complete
  `Simulation` when practical.
- Cross-domain behavior uses explicit commands, queries, or events.
- Pull requests introducing a trait identify its current implementations or
  testing boundary.
- Determinism and domain invariants take precedence over pattern conformity.

## Consequences

- Architectural review has explicit criteria without requiring every module to
  share the same shape.
- Small functions and concrete types remain acceptable where they communicate
  intent better than traits.
- Some extension points appear later, after repeated requirements reveal the
  correct abstraction.
