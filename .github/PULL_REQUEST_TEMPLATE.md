## Goal

<!-- One observable behavior or narrowly scoped change. -->

Closes #

## Roadmap

<!-- Phase and checkbox this PR advances. Update ROADMAP.md if it closes the item. -->

## Behavior

### Before

### After

## Causality and determinism

<!-- What knowledge does the agent use? What is the causal chain? Is the result stable for the same seed? -->

## Performance

<!-- Frequency, expected complexity, new scans/pathfinding, and measured impact when relevant. -->

## Tests

<!-- Tests added or changed, including same-seed coverage when relevant. -->

## Validation scenario

<!-- The smallest emergent story that demonstrates the behavior end to end. -->

## Definition of done

- [ ] The change implements one observable behavior.
- [ ] Agents use only information available to them.
- [ ] Same seed and inputs produce the same result.
- [ ] No unnecessary global scan or per-tick pathfinding was added.
- [ ] The behavior is observable or inspectable where appropriate.
- [ ] `./scripts/check.ps1` passes locally.
- [ ] CI is green.
- [ ] `ROADMAP.md` is updated if this closes a roadmap item.
