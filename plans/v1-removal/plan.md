# V1 Removal & Cleanup Plan

## Goal
Retire the v1 fixture/audit/pawn-fixture machinery so v2 is the only world-spawning path. Salvage the parts worth keeping (composition layering invariants, optional asset-decodability probe). Apply the smaller bit-rot cleanups (broken tests, unused hediff scaffolding, stale CLI, unused import) along the way.

End state: one fixture command (v2 + RON), one binary code path from CLI → world → render, no broken tests, no speculative scaffolding for features that don't exist yet.

## Why now
- v1's procedural scene generator predates RON fixtures and is now strictly worse than `fixture v2 --scene <ron>`.
- `fixture v1`, `fixture pawn`, and `audit` share ~95% of their code via `generate_v1_scene_fixture`; they are flag-distinguished modes of the same thing.
- v2 is the actual game-loop foundation per `docs/vision.md`. Two ways to spawn a world is tax we don't want while building real sim systems on top of v2.
- Two of the three test files for v1 are already broken (assert on log lines that don't exist or use obsolete CLI flags).

## Salvage list (things v1 does that we want to keep somewhere)
1. **Composition layering invariants** (`validate_pawn_focus`, `measure_head_body_delta_y`, `validate_basic_pawn_layering` in `commands/fixture_cmd.rs`). These check `head_z > body_z`, `hair_z > head_z`, `beard_z > body_z`, and head_y above body_y — properties of `compose_pawn`, not of a render pipeline.
2. **Asset decodability sweep** (the `used_fallback` filter loops in `generate_v1_scene_fixture`). Currently entangled with scene generation but doubles as a coarse health probe for "do we have textures for this category of def."
3. **Visual N-pawn grid** (audit's `pawn_audit_mode`). Useful for eyeballing loadouts side-by-side. Not common enough to be load-bearing.

## Discard list
- `commands/fixture_cmd.rs` (the v1+pawn+audit dispatcher and `generate_v1_scene_fixture` body)
- `src/scene.rs` (`generate_fixture_map`, `FixtureMap`, `ThingInstance`, `PawnInstance`) — single caller, dies with v1
- `cli::Command::Audit`, `cli::AuditCmd`
- `cli::FixtureCmd::V1`, `cli::FixtureCmd::Pawn`, `cli::FixtureSceneCmd`
- `tests/v1_smoke.rs` — broken (asserts on missing log line)
- `tests/v1_golden.rs` — v1 gone
- `tests/pawn_fixture_golden.rs` — broken (uses obsolete `--pawn-fixture` flag)
- `tests/golden/v1_fixture_256.png`
- Hediff overlay scaffolding: `pawn/rules.rs`, `OverlayAnchor` enum, `HediffOverlayInput`, `PawnRenderInput::hediff_overlays`, `PawnNodeKind::Hediff` (if only hediff-related), `NodePayload::Hediff`, the overlay branches in `pawn/compose.rs` (lines ~110, ~230, ~444). No production caller populates it; only compose.rs internal tests exercise it. ~80 LOC of speculative scaffolding.
- Unused `Vec2` import at `src/runtime/v2/mod.rs:240`
- `OverlayAnchor::Body` (already flagged dead by compiler — falls out with hediff removal)

## Settled decisions
- **Asset decodability probe:** add a `debug probe-defs` command (Step 4).
- **CLI shape:** collapse `fixture v2 --scene <path>` to positional `fixture <path>`. Drop the `FixtureCmd` enum entirely.
- **`pawn/graph.rs` inline:** separate follow-up, not part of this plan.
- **Branch:** work happens on `main` (already there; no prior apparel-scale commits to wait on).

## Step plan

Each step ends with `cargo build && cargo test && cargo clippy` green. One commit per step (all on a new branch off main, since current branch is the apparel-scale work).

### Step 0 — Branch
```
git checkout -b v1-removal
```
(Already on `main`.)

### Step 1 — Lift composition invariants to unit tests
Move `validate_pawn_focus`, `measure_head_body_delta_y`, `validate_basic_pawn_layering` out of `commands/fixture_cmd.rs` and into the existing `#[cfg(test)] mod tests` block in `src/pawn/compose.rs`. Each becomes a `#[test]` fn that builds a synthetic `PawnRenderInput` (the test module already has a builder pattern at compose.rs:278-300) and asserts on the resulting `PawnComposition.nodes`.

Rationale: these are properties of `compose_pawn`, not of the render pipeline. They should run on every `cargo test`, not only when someone happens to invoke `fixture pawn`.

**Files:** `src/pawn/compose.rs` (add tests), `src/commands/fixture_cmd.rs` (no removal yet — that's Step 5).

**Verify:** `cargo test --lib pawn::compose` shows the new tests pass.

### Step 2 — Remove hediff scaffolding
Delete in this order to avoid breaking intermediate states:
1. `src/pawn/rules.rs` — entire file
2. `pub mod rules` line in `src/pawn/mod.rs`
3. `PawnNodeKind::Hediff` variant in `src/pawn/tree.rs` (verify it's only hediff-used, not also a stump fallback or similar)
4. `NodePayload::Hediff(...)` in `src/pawn/graph.rs`
5. `OverlayAnchor` enum and `HediffOverlayInput` struct in `src/pawn/model.rs`
6. `PawnRenderInput::hediff_overlays` field in `src/pawn/model.rs`
7. The two hediff branches in `src/pawn/compose.rs` (the `for overlay in &input.hediff_overlays` loop ~line 110 and the apparel-skip-when-overlay logic ~line 230)
8. Any test in compose.rs that constructs `HediffOverlayInput` (around line 444)
9. Strip `hediff_overlays: Vec::new()` lines from every `PawnRenderInput { ... }` literal (3 known sites: `commands/fixture_v2_cmd.rs:290`, `runtime/v2/mod.rs:270`, compose.rs internal test at ~300)

**Files:** `src/pawn/{rules.rs,mod.rs,tree.rs,graph.rs,model.rs,compose.rs}`, `src/commands/fixture_v2_cmd.rs`, `src/runtime/v2/mod.rs`.

**Verify:** `cargo build && cargo clippy` — the `OverlayAnchor::Body` dead-code warning should disappear.

### Step 3 — Remove unused `Vec2` import
Single-line cleanup: `src/runtime/v2/mod.rs:240` — change `use glam::{Vec2, Vec3};` to `use glam::Vec3;`.

(Standalone tiny commit, or fold into Step 2's commit if cleaner.)

### Step 4 — Add `debug probe-defs`
Add a new `DebugCmd::ProbeDefs` variant in `src/cli.rs`. Implementation in `src/commands/debug_cmd.rs` walks `defs.body_type_defs`, `head_type_defs`, `hair_defs`, `beard_defs`, `apparel_defs`, calls `asset_resolver.resolve_texture_path` on each, and prints per-category `<decoded>/<total>` counts. Pattern after the existing `ProbeTerrain` and `PackedDecodeProbe` commands.

**Files:** `src/cli.rs`, `src/commands/debug_cmd.rs`, `README.md` (add to debug commands list).

**Verify:** `cargo run -- debug probe-defs` runs and emits per-category counts when RimWorld data is configured.

### Step 5 — Tear out v1, audit, pawn-fixture
1. **CLI** (`src/cli.rs`):
   - Remove `Command::Audit(AuditCmd)` variant and `AuditCmd` struct
   - Drop the `FixtureCmd` enum entirely (all three variants gone)
   - Remove `FixtureSceneCmd` struct
   - Change `Command::Fixture { mode: FixtureCmd }` → `Command::Fixture(FixtureV2Cmd)`. Consider renaming `FixtureV2Cmd` → `FixtureCmd` (struct) since the v2 distinction is now meaningless.
   - Change `scene` arg from `--scene <path>` flag to positional `<path>` in the renamed struct.
2. **Dispatcher** (`src/commands/mod.rs`):
   - Drop `mod fixture_cmd;` line
   - `dispatch` match arms: drop `Audit`, simplify `Fixture` to call `fixture_v2_cmd::run_fixture_v2` directly
   - Decide naming: rename `commands/fixture_v2_cmd.rs` → `commands/fixture_cmd.rs` now that it's the only one. Update `mod` line. (Optional but tidier.)
3. **Files to delete:**
   - `src/commands/fixture_cmd.rs` (the v1 dispatcher — assuming we did the rename above, reverse the order: delete v1 file first, then rename v2 file)
   - `src/scene.rs`
   - `pub mod scene;` line in `src/main.rs` (if present — verify)
4. **Verify imports cleaned up:** grep for `crate::scene` and `commands::fixture_cmd::run_fixture` — should be zero hits.

**Files:** `src/cli.rs`, `src/commands/mod.rs`, `src/commands/fixture_cmd.rs` (delete), `src/commands/fixture_v2_cmd.rs` (rename), `src/scene.rs` (delete), `src/main.rs` (mod line).

**Verify:** `cargo build && cargo clippy && cargo test` all green. `cargo run -- fixture v2 --scene fixtures/v2/move_lane.ron --no-window` still works.

### Step 6 — Test cleanup
Delete:
- `tests/v1_smoke.rs`
- `tests/v1_golden.rs`
- `tests/pawn_fixture_golden.rs`
- `tests/golden/v1_fixture_256.png`
- `tests/golden/` directory if empty

Keep:
- `tests/v0_smoke.rs` (`render --thingdef Steel` — still works, smallest sanity check)
- `tests/v2_smoke.rs` (active path)

**Verify:** `cargo test` runs only `v0_smoke` and `v2_smoke` (plus lib unit tests including the new compose layering tests from Step 1).

### Step 7 — README update
Strip v1/audit/pawn sections from `README.md`. Replace with:
- Quick start showing `cargo run -- fixture <ron-path>` (or `fixture v2 --scene` if we kept the subcommand)
- Updated debug commands list (add `probe-defs` if added)
- Updated test section (drop v1_smoke/v1_golden references)

**Files:** `README.md`.

### Step 8 — Final sweep
- `cargo build --release` to confirm release builds
- `cargo clippy -- -D warnings` to confirm no warnings remain
- Manual: `cargo run -- fixture fixtures/v2/move_lane.ron` opens the window, click works, pawn moves
- Manual: `cargo run -- fixture fixtures/v2/obstacle_pathing.ron` works
- Manual: `cargo run -- fixture fixtures/v2/mixed_things_pawns.ron` works

## Risks & gotchas
- **`PawnNodeKind::Hediff`**: need to confirm it's not also used for stumps or any other non-hediff render. Quick grep before deleting.
- **`pub mod scene`** may not even exist in main.rs — could be in lib root via implicit binary structure. Verify before assuming the line is there.
- **Renaming `fixture_v2_cmd.rs` → `fixture_cmd.rs`** has to happen *after* deleting the original `fixture_cmd.rs`, otherwise filesystem collision. Doable in one step but order matters in commands.
- **The apparel-scale branch is unpushed.** This plan assumes we branch from `main`. If apparel-scale work hasn't shipped yet, those 7 commits need to land first or this plan needs to rebase onto that branch. **Confirm with Joe before Step 0.**
- **`fixture` positional arg migration:** any dev scripts or shell history using `fixture v2 --scene path` will break. Worth a quick `grep -r "fixture v2"` across any notes or scripts before landing.

## What this plan does NOT do
- Does not collapse `pawn/graph.rs` into `pawn/compose.rs` (Q3 — separate follow-up).
- Does not rework `defs.rs` (976 LOC, but still earning its keep).
- Does not split `commands/common.rs` (446 LOC, mostly genuine shared helpers).
- Does not touch the apparel-scale branch's commits.

## Estimated diff size
- Net deletion: ~1,200–1,400 LOC (most of `commands/fixture_cmd.rs` at 659, all of `scene.rs` at 99, three test files ~200, hediff scaffolding ~80, CLI cruft ~50, README sections)
- Net addition: ~50 LOC (new compose tests in Step 1, optional probe-defs in Step 4)
- Roughly 8 commits, one per step (Step 3 may fold into 2).
