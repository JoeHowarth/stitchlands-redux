# AGENTS.md

## Lint Policy

- Do not add local/manual Clippy allowances such as `#[allow(clippy::...)]` on functions, modules, or items.
- Preferred order:
  1. Fix the underlying issue.
  2. If a rule must be relaxed, change lint policy globally/invocation-level (for example in project-wide lint config or clippy command flags), not per-item.

## Work Completion Policy

- After each piece of work, run formatting, tests, and lint checks.
- Fix any lint findings instead of suppressing them locally.
- Commit the completed piece of work once checks are passing.
