# AGENTS.md

## Commands

- `cargo fmt --all` — format. Toolchain pinned to nightly via `rust-toolchain.toml`.
- `cargo clippy --all-targets -- -D warnings` — lint. Warnings are errors.
- `cargo test` — unit tests.
- `cargo run -- render-fixtures` — render every fixture RON to `fixtures/renders/<stem>.png`. The visual regression gate per Work Completion Policy. Skips writing unchanged outputs.
- `cargo run -- fixture <path>` — render a single fixture; add `--no-window --screenshot <path>` for headless capture.

## Module Map

Top-level layout under `src/`:

- `renderer/` — wgpu pipeline construction, GPU resource ownership, frame execution. Submodules: `gpu_context`, `textures`, `pipelines`, `frame`, `camera`, `screenshot`.
- `world/` — `WorldState` simulation data + per-tick stepping; sibling `RenderState` holds derived per-tick render inputs (fog, snow, sky, shadow vector).
- `runtime/v2/` — live tick loop, fixed-step state machine, per-tick draw bridge.
- `commands/` — per-CLI-command scene assembly (fixture rendering, batch render, overlay builders).
- `pawn/` — pawn render graph: `compose_pawn` pipeline, layered sprite emission.
- `path/` — opaque `PathGrid` + `find_path`.
- `assets/` — texture resolution against loose files and packed Unity bundles.
- `defs.rs` — RimWorld XML def parsers (`ThingDef`, `TerrainDef`, `ApparelDef`, etc.).
- `linking.rs` — RimWorld edge-linking adjacency rules.
- `interaction/` — input state, mouse picking, cell ↔ world coordinate conversion.
- `fixtures/` — RON scene schema, loader, validator.
- `viewer.rs` — winit `ApplicationHandler`, event loop, runtime ↔ renderer bridge.
- `app_context.rs` — shared dependency bundle (defs, asset resolver, configs).
- `cli.rs`, `main.rs` — entry points.

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
- After every commit, render all fixture RON files with
  `cargo run -- render-fixtures`; outputs must go under `fixtures/renders/`
  with filenames matching the source fixture stems.
- Fix any lint findings instead of suppressing them locally.
- Commit the completed piece of work once checks are passing.

## Path Reference Policy

- Use repository-relative paths in communication (for example `src/renderer/mod.rs`), not absolute system paths.

## Commit Conventions

- **Messages**: terse imperative ("Render fog with material texture", "Skip unchanged screenshot writes"). One-line subject; body only when reasoning isn't obvious from the diff.
- **Branches**: `feat/<topic>` for features, `fix/<topic>` for bug fixes (e.g. `feat/water-rendering`, `fix/thingdef-inheritance`).
- Do not mention Claude or Anthropic in commit messages or code comments.

## RimWorld Porting Policy

- When decompiled RimWorld source is available for a system, prefer a direct port of the system boundary over a visually plausible substitute.
- Preserve RimWorld's authored inputs, runtime state, mesh topology, material colors, shader uniforms, neighbor rules, and silhouette rules before adding renderer-specific adapters.
- If the exact Unity shader or section-mesh infrastructure is not available yet, keep any fallback narrow, clearly named as temporary, and shaped around the same RimWorld data and mesh semantics.
- Treat the static sun shadow bug as the cautionary example: CPU-extruding every footprint side looked plausible at full view, but diverged from `SectionLayer_SunShadows` and produced stacked dark triangles when zoomed.

## RimWorld Data Naming

Fixture RON fields like `body`, `head`, `hair`, `beard` take XML **defNames**, not texture path segments. These often differ:

- defName `Male_AverageNormal` vs graphicPath `.../Male_Average_Normal`
- defName `Full` vs graphicPath `.../Beard_Full`

Source of truth is `RimWorldMac.app/Data/Core/Defs/`, distinct from the packed Unity assets the resolver loads. The `choose_*_def` functions in `src/commands/` warn on miss — watch the logs. `PawnSpawn` in `src/fixtures/schema.rs` documents this on its doc comment; cross-check before authoring new pawn fixtures.

## Plans

- See `plans/README.md` for the plan-folder lifecycle (active vs `plans/archive/`, status convention, where deferred items go).
- `plans/BACKLOG.md` is the single entry point for deferred work that doesn't warrant its own plan folder yet.
- A plan folder's presence under `plans/` is not a completion signal on its own — verify against `git log` and the code before starting work.

## Worktree Policy

- Worktrees live under `.claude/worktrees/<name>/`. They are short-lived.
- After a worktree's branch is merged to `main`, delete the worktree (`git worktree remove .claude/worktrees/<name>`). Don't leave merged worktrees sitting around.

## External References

- **RimWorld decompiled C# source**: `~/rimworld-decompiled/`. Start at `~/rimworld-decompiled/MAP/INDEX.md` — a reference map with per-subsystem pages (pawn rendering, graphics primitives, defs/loading, components, jobs/AI, map/world) and file:line citations into the frozen codebase. Use this when reverse-engineering game behavior or algorithms.
- **RimWorld extracted Unity shaders**: `~/rimworld-shader-extract/` — AssetRipper exports from the Steam macOS build. Use `assetripper-1.3.5-disassembly-export/ExportedProject/Assets/Resources/materials/` for Metal shader subprogram disassembly, `assetripper-1.3.5-decompile-export/` for ShaderLab shells with properties/render states, and `assetripper-yaml-export/` for serialized shader metadata. Treat these as reference material for reimplementation, not code to vendor into this repo.
- **RimWorld XML defs**: `RimWorldMac.app/Data/Core/Defs/` — game data, distinct from the decompiled engine/logic source above.
