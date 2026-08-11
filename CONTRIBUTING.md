# Contributing to Nexus

Nexus uses the roadmap for direction and GitHub Issues for executable work. Keep changes small enough that each pull request introduces one observable behavior.

## Workflow

1. Choose the next vertical slice from `ROADMAP.md`.
2. Create an Issue with testable acceptance criteria.
3. Create a branch from the latest `main`.
4. Open a draft pull request after the first functional commit.
5. Implement and validate the behavior.
6. Run `./scripts/check.ps1` from the repository root.
7. Mark the pull request ready when CI is green and the validation scenario works.
8. Squash merge, delete the branch, and close the Issue.
9. Update `ROADMAP.md` in the same pull request that completes a roadmap item.

Suggested branch names include `feat/social-positive-targets`, `fix/social-memory-retry`, and `chore/development-workflow`.

## Pull request scope

Aim for one pull request per observable behavior. Refactoring or tests needed for that behavior belong in the same pull request; unrelated behavior belongs in another Issue and branch.

Avoid stacked pull requests by default. If a stack is genuinely useful:

1. Open the parent pull request first.
2. Point the child pull request at the parent branch and make the dependency explicit.
3. Merge the parent before the child.
4. Retarget the child to `main`, update it, and wait for required CI before merging.

CI runs for every pull request base so stacked work is still verified, but only `main` is expected to remain permanently releasable.

## Local validation

The validation entrypoint mirrors CI:

```powershell
./scripts/check.ps1
```

For a narrower local iteration loop:

```powershell
./scripts/check.ps1 -Target engine
./scripts/check.ps1 -Target web
```

The full command checks Rust formatting, native and WASM clippy, Rust tests, the WASM build, dependency installation, TypeScript types, and the production web build.

## Definition of done

A Nexus behavior is complete when:

- the behavior and its causal chain are implemented;
- agents use only information available to them;
- results are deterministic for the same seed and inputs;
- automated tests cover the behavior;
- the result is observable or inspectable where appropriate;
- no unnecessary global scan or per-tick pathfinding was introduced;
- performance impact and update frequency are understood;
- the validation scenario works end to end;
- CI is green; and
- the relevant roadmap item is updated.
