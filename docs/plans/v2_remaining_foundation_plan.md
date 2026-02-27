# v2 Remaining Foundation Plan (Post-Implementation Reality)

## Intent

This plan updates the original v2 implementation sequence based on the code that now exists.

The immediate objective is to preserve current working behavior (hover/select/click-to-move in `fixture v2`) while refactoring toward clean ownership boundaries and a durable runtime foundation for v3+ features.

## Current Reality Snapshot

What is already working:

- RON fixture schema + validation + sample scenes.
- `fixture v2 --scene ...` command path.
- Runtime world spawn (`WorldState`) with pawns/things/terrain.
- A* pathing + world tick primitives in `src/world/tick.rs`.
- Renderer static/dynamic sprite split.
- Interactive window loop: hover, selection, click-to-move, moving pawns, path intent overlays.

What is currently coupled and should be cleaned up:

- Runtime simulation and interaction state live mostly inside `src/viewer.rs`.
- World movement logic is duplicated (viewer-local runtime tick vs `world::tick_world`).
- `fixture_v2_cmd` still does rendering composition and runtime hint assembly.
- Dynamic rendering still moves prebuilt pawn nodes by position delta rather than rebuilding `PawnRenderInput` from runtime state each frame.
- Renderer owns texture registration but submission still passes image payloads each frame.

## Target End-State Architecture

The clean v2 endpoint should look like this:

1. `commands/fixture_v2_cmd.rs`
- Loads fixture and defs.
- Builds a `V2RuntimeBootstrap` object.
- Launches viewer/runtime host.
- No per-frame gameplay logic.

2. `runtime/v2/` (new)
- Owns mutable runtime state and update loop API.
- Single source of truth for:
  - world simulation,
  - interaction state,
  - move intent issuance,
  - frame dynamic draw output.

3. `world/`
- Owns simulation state + deterministic stepping primitives.
- No input/window/pixel concerns.

4. `interaction/`
- Owns picking + interaction state transitions.
- No rendering calls.

5. `render bridge` (new module)
- Converts runtime state to `SpriteDraw`/dynamic draw data.
- Stable ordering and overlay ordering in one place.

6. `viewer.rs`
- Thin host that forwards input/events to runtime and submits draw data to renderer.
- No game rule decisions.

## Design Constraints

- Keep current user-visible behavior working after each phase.
- Prefer additive migration with small reversible commits.
- Preserve deterministic fixed-step semantics for debugability.
- Keep v1 and non-v2 command behavior unchanged.

## Ordered Task Plan

## Phase R1: Introduce Runtime Core Type (No Behavior Change)

1. Add `src/runtime/v2/mod.rs` with:
- `V2Runtime` struct (world + interaction + render cache),
- `V2RuntimeConfig`,
- `V2FrameOutput`.

2. Move runtime-only structs out of `viewer.rs`:
- pawn runtime fields,
- selected pawn state,
- path overlay state.

3. Convert viewer launch contract:
- viewer receives `V2Runtime` handle/bootstrap instead of ad-hoc `runtime_hints`.

4. Keep output identical to current visuals and input behavior.

Exit criteria:

- `viewer.rs` no longer defines gameplay runtime structs.
- Existing v2 window interaction still works exactly as now.

## Phase R2: Unify Simulation Path Through `world` Module

1. Delete viewer-local movement simulation logic.

2. Route all move issuance through `world::issue_move_intent`.

3. Route all ticking through `world::tick_world`.

4. Add world utility methods needed by runtime:
- selected pawn query helpers,
- occupancy checks for click dispatch,
- optional “is idle” helpers.

5. Add tests for blocked destination, zero-length path, and repeated move reissue.

Exit criteria:

- Single movement implementation path exists.
- No duplicate movement math in viewer/runtime glue.

## Phase R3: Interaction State Machine Extraction

1. Expand `interaction::InteractionState` to hold:
- `hovered_cell`,
- `selected_pawn_id`,
- last issued destination / intent data.

2. Add pure handlers in `src/interaction/input.rs`:
- `on_cursor_moved`,
- `on_left_click`,
- `on_right_click`,
- `on_escape`.

3. Handler returns explicit actions (e.g. `SelectPawn`, `IssueMove`, `ClearSelection`, `NoOp`) consumed by runtime.

4. Keep picking in `interaction/picking.rs` and make it runtime-agnostic.

Exit criteria:

- Viewer forwards events; interaction module decides intent.
- Interaction logic can be unit-tested without window/renderer.

## Phase R4: Render Bridge Consolidation

1. Add `src/runtime/v2/render_bridge.rs`.

2. Move dynamic sprite generation (pawn transforms + overlays) out of viewer into bridge.

3. Introduce explicit ordered passes in bridge:
- hover overlay,
- selection overlay,
- path intent markers,
- pawn dynamic layers.

4. Add deterministic stable sort key API for dynamic draw items.

Exit criteria:

- Viewer submits draw output; no overlay assembly in viewer.
- Dynamic ordering is stable in repeated runs.

## Phase R5: True `PawnState -> PawnRenderInput` Per Frame

1. Introduce static pawn visual profile type:
- body/head/hair/beard/apparel defs,
- render tuning defaults.

2. Runtime derives `PawnRenderInput` per pawn per frame from:
- mutable `PawnState` (position/facing/path progress),
- static visual profile.

3. Compose pawn nodes every frame via existing `compose_pawn` path.

4. Remove delta-shift approach of prebuilt pawn node sprites.

Exit criteria:

- Pawn visual output is frame-derived from runtime state.
- Future facing changes/animation hooks become straightforward.

## Phase R6: Complete Texture Handle Submission Path

1. Add explicit draw record type using `TextureId` + sprite params.

2. Runtime/render bridge emits draw records, not `RgbaImage` payloads.

3. Renderer registration becomes startup/cache event; dynamic frame work updates instance data only.

4. Maintain full dynamic resubmit strategy in v2 (no partial dirty updates yet).

Exit criteria:

- No per-frame image cloning for dynamic draw submission.
- Draw path is aligned with planned `TextureId` architecture.

## Phase R7: Deterministic Headless Runtime Mode

1. Add command flags for deterministic run control:
- `--ticks <N>`,
- `--fixed-dt <secs>`,
- `--screenshot <path>` with no window.

2. Emit runtime counters/log summaries on completion.

3. Ensure repeat runs produce stable structural outputs.

Exit criteria:

- Closed-loop agent debugging possible without manual window interaction.

## Phase R8: Validation and Regression Hardening

1. Add structural integration tests for:
- select pawn then move,
- path exists/does not exist cases,
- pawn reaches destination within bounded ticks,
- selection persistence/deselect semantics.

2. Add tolerant screenshot tests for key v2 scenes:
- move lane,
- obstacle pathing.

3. Add assertions for static-vs-dynamic layer ownership:
- terrain/things only in static,
- pawns/overlays only in dynamic.

Exit criteria:

- Regressions in interaction/movement are caught automatically.
- Visual drift checks are present but not brittle.

## Cross-Cutting Complexity Management Rules

1. Keep command glue thin.
- No behavior logic in `commands/*` beyond setup/launch.

2. Keep viewer thin.
- No pathfinding decisions in event handlers.

3. Keep runtime deterministic.
- Fixed-step updates only; no frame-time simulation shortcuts.

4. Keep conversion boundaries explicit.
- World state != interaction state != render draw state.

5. Keep each phase shippable.
- `cargo fmt`, `cargo test`, `cargo clippy --all-targets -- -D warnings` after every phase.

## Suggested PR/Commit Sequence

1. R1 + R2 together if refactor is small; otherwise separate.
2. R3 as isolated interaction state-machine change.
3. R4 + R5 in two commits (bridge first, per-frame composition second).
4. R6 renderer handle migration.
5. R7 deterministic headless.
6. R8 test and validation hardening.

## Updated v2 Completion Criteria

v2 is considered complete when:

- Interaction and movement runtime logic are not viewer-owned.
- Click-to-move pathing remains stable and deterministic.
- Pawn rendering is derived each frame from runtime state.
- Dynamic draw submission uses texture handles cleanly.
- Deterministic headless validation path exists.
- Structural and tolerant visual regressions are covered in tests.
