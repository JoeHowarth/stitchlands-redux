# AGENTS.md

## Lint Policy

- Do not add local/manual Clippy allowances such as `#[allow(clippy::...)]` on functions, modules, or items.
- Preferred order:
  1. Fix the underlying issue.
  2. If a rule must be relaxed, change lint policy globally/invocation-level (for example in project-wide lint config or clippy command flags), not per-item.
