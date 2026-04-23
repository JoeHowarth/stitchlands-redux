# AGENTS.md

## Lint Policy

- Do not add local/manual Clippy allowances such as `#[allow(clippy::...)]` on functions, modules, or items.
- Preferred order:
  1. Fix the underlying issue.
  2. If a rule must be relaxed, change lint policy globally/invocation-level (for example in project-wide lint config or clippy command flags), not per-item.

## Debugging Workflow

- Use good judgment when running verification commands.
- Do not run `clippy` + full test suites after every small debugging edit.
- During iterative debugging, prefer targeted checks; run full lint/test sweeps at logical checkpoints or before finalizing.
- Prefer closed-loop debugging when possible:
  - run with deterministic screenshot output,
  - inspect generated images/logs directly,
  - iterate without requiring user confirmation after every small step.

## Work Completion Policy

- After each piece of work, run formatting, tests, and lint checks.
- Fix any lint findings instead of suppressing them locally.
- Commit the completed piece of work once checks are passing.

## Path Reference Policy

- Use repository-relative paths in communication (for example `src/renderer.rs`), not absolute system paths.

## Plans

- See `plans/README.md` for the plan-folder lifecycle (active vs `plans/archive/`, status convention, where deferred items go).
- `plans/BACKLOG.md` is the single entry point for deferred work that doesn't warrant its own plan folder yet.
- A plan folder's presence under `plans/` is not a completion signal on its own — verify against `git log` and the code before starting work.

## Worktree Policy

- Worktrees live under `.claude/worktrees/<name>/`. They are short-lived.
- After a worktree's branch is merged to `main`, delete the worktree (`git worktree remove .claude/worktrees/<name>`). Don't leave merged worktrees sitting around.

## External References

- **RimWorld decompiled C# source**: `~/rimworld-decompiled/`. Start at `~/rimworld-decompiled/MAP/INDEX.md` — a reference map with per-subsystem pages (pawn rendering, graphics primitives, defs/loading, components, jobs/AI, map/world) and file:line citations into the frozen codebase. Use this when reverse-engineering game behavior or algorithms.
- **RimWorld XML defs**: `RimWorldMac.app/Data/Core/Defs/` — game data, distinct from the decompiled engine/logic source above.
