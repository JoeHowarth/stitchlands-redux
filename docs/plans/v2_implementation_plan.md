# v2 Implementation Plan

## Intent

Translate the high-level v2 goals into concrete architecture and ordered implementation tasks:

- believable pawn movement,
- first interactions (selection, hover, click-to-move path intent),
- data-driven fixture scenes in RON.

## Architecture (v2)

## Up-Front Design Decisions

1. Dynamic renderer strategy (required decision before interaction work):
   - Chosen v2 approach: split draw submission into static + dynamic lists.
     - Static list: terrain + static things, built once (or rebuilt only on scene edit/load).
     - Dynamic list: pawns + interaction overlays, rebuilt each frame.
   - Dynamic upload strategy in v2: full dynamic buffer rewrite per frame.
   - Deferred optimization: partial dirty-buffer updates only if profiling requires it.

2. Static-vs-runtime scene model:
   - `SceneFixture`/`FixtureMap` remains immutable loaded data.
   - `WorldState` is mutable runtime state derived from fixture data.
   - v1 command paths remain on existing builders until v2 path is stable.

3. Def loading remains unchanged:
   - RimWorld defs continue to load from XML (`roxmltree` path stays as-is).
   - RON is only for local scene fixture authoring.

4. New dependencies for fixture path:
   - Add `serde` + `serde_derive` + `ron` for fixture schema parsing.

### 1) Fixture data layer

- Add `fixtures/` directory for scene files (`*.ron`).
- Introduce `src/fixtures/` module with:
  - schema structs (`SceneFixture`, `MapSpec`, `ThingSpawn`, `PawnSpawn`, `CameraSpec`),
  - parser/validator,
  - loader API (`load_fixture(path) -> SceneFixture`).
- Keep fixture references def-name based (`terrain_def`, `thing_def`).
- Pawn preset shape for v2 (fixture-local, not external def):
  - `PawnSpawn` includes a lightweight inline preset/data block (or equivalent fields) for:
    - body/head/hair/beard identifiers,
    - facing,
    - optional apparel def-name list,
    - optional label/debug name.
  - This is a fixture schema concept only; no new RimWorld `PawnPreset` def type is introduced.

### 2) Runtime world state (minimal)

- Add `src/world/` module with concrete state structs:
  - `WorldState` (map + entities),
  - `PawnState` (position, target path, move progress),
  - `ThingState` (position, static for v2).
- World state is built from fixture data at startup.
- Explicit bridge:
  - `PawnState` stores mutable runtime fields (position/path/progress/facing intent).
  - `PawnRenderInput` remains render composition input derived each frame from `PawnState` + static pawn fixture/profile data.

### 3) Interaction state

- Add `src/interaction/` module:
  - `InteractionState { hovered_cell, selected_entity, move_intent }`,
  - input handlers for click/hover/select,
  - translation between screen and world cell coords.

### 3a) Camera picking / coordinate transform

- Add explicit screen-to-world picking utility as a prerequisite for interactions:
  - viewport pixel -> world position -> map cell.
- Keep this utility independent of selection logic to simplify testing.

### 4) Movement + pathfinding

- Add `src/path/` module with grid A* for v2 fixtures.
- Add explicit passability model in fixture/runtime data:
  - terrain passability class (walkable/blocking),
  - thing occupancy/blocking flag (`blocks_movement` in fixture spawns for v2),
  - pawn occupancy handling rules.
- Movement model:
  - click destination -> path cells,
  - pawn follows path with fixed step rate,
  - interpolation for visual smoothness.
- No jobs/tasks system in v2; only direct move intent.

### 5) Rendering pipeline changes

- Keep existing terrain/things/pawn drawing pipeline, but split submission sources:
  - `set_static_sprites(Vec<SpriteDraw>)`
  - `set_dynamic_sprites(Vec<SpriteDraw>)`
- Add a fixed-step runtime loop for v2 fixture path:
  - world tick at fixed dt (target 60 Hz),
  - rebuild dynamic sprites from world + interaction state each frame,
  - render with current camera.
- Add interaction overlays pass (in this order):
  1. hover highlight,
  2. selection marker,
  3. path intent line/cell markers.
- Keep deterministic sort key for moving entities.

### 6) Renderer API adjustments

- Introduce stable texture registration + references for draw items:
  - `TextureId` returned by texture registration.
  - Draw records reference `TextureId` instead of raw image payloads.
- Maintain one pipeline initially; avoid multi-pipeline expansion in v2.
- Keep v1 command paths compatible through adapter helpers.
- Phase placement: these API adjustments are implemented during **Phase B2**.

## Ordered Task Plan

### Phase A: Fixture system foundation

1. Create fixture schema structs + RON parser.
   - Add serde/ron dependencies.
2. Add fixture validation command (`debug validate-fixture <path>`).
3. Provide 2-3 micro-scene fixture files:
   - movement lane,
   - obstacle pathing,
   - mixed things + pawns.

Exit criteria:
- fixture files load and validate,
- invalid fixture shows actionable errors.

### Phase B1: World runtime from fixture (static)

1. Build `WorldState` from fixture data.
2. Replace hardcoded fixture builders on one command path (`fixture v2`).
3. Keep existing v1 paths untouched.

Exit criteria:
- `fixture v2 --scene <file.ron>` renders terrain/things/pawns from data.

### Phase B2: Dynamic frame loop

1. Implement per-frame world tick + redraw cycle for `fixture v2`.
2. Wire static/dynamic sprite submission split in renderer.
3. Implement renderer API changes (`TextureId` registration + draw records using handles).
4. Adopt full dynamic resubmit path in v2 loop.
5. Add camera-independent frame counters/timing logs for closed-loop debugging.
6. Add deterministic fixed-step simulation mode for headless/debug runs.

Exit criteria:
- dynamic sprite list is rebuilt each frame without visual regressions vs B1 static output.
- dynamic ordering remains stable frame-to-frame.
- static layer is not rebuilt every frame.

### Phase C: Interaction loop

1. Implement screen-to-world/cell picking utility (`interaction/picking.rs`).
2. Add hover cell tracking.
3. Add click-to-select entity.
4. Add click ground to issue move for selected pawn.

Exit criteria:
- selecting and hover are visible,
- move intent is generated and stored.

### Phase D: Pathing + movement

1. Implement A* grid path search with obstacle occupancy.
2. Move selected pawn along path over time.
3. Render path intent overlay.
4. Validate `PawnState` -> `PawnRenderInput` conversion each frame.

Exit criteria:
- click-to-move works end-to-end on fixture scenes,
- pawns animate along path with stable ordering.

### Phase E: Validation + regression

1. Add structural integration tests for fixture load + movement invariants.
2. Add tolerant screenshot checks for key micro-scenes.
3. Add a deterministic "headless tick run" command for closed-loop debugging.

Exit criteria:
- tests catch broken interaction/movement behavior,
- iteration loop is reproducible without manual steps.

## Suggested Module Layout

- `src/fixtures/mod.rs` (`schema.rs`, `loader.rs`, `validate.rs`)
- `src/world/mod.rs` (`state.rs`, `spawn.rs`, `tick.rs`)
- `src/interaction/mod.rs` (`state.rs`, `input.rs`, `selection.rs`)
- `src/interaction/picking.rs` (screen-to-world + cell picking)
- `src/path/mod.rs` (`astar.rs`, `grid.rs`)
- `src/commands/fixture_v2_cmd.rs` (v2 fixture command runner)
- `src/renderer.rs` (static/dynamic submission API additions)

## Command Surface (planned)

- `cargo run -- fixture v2 --scene fixtures/v2/move_lane.ron`
- `cargo run -- debug validate-fixture fixtures/v2/move_lane.ron`
- `cargo run -- fixture v2 --scene fixtures/v2/move_lane.ron --no-window --screenshot target/v2_move_lane.png`

## Risks / Mitigations

- Pathing correctness drift:
  - keep fixture maps tiny and deterministic first.
- Render/input coupling complexity:
  - isolate `InteractionState` updates from draw code.
- Dynamic redraw cost from full resubmit path:
  - keep fixture scale small in v2 and add profiling checkpoint before feature growth.
- Static/dynamic split drift (wrong layer assignment):
  - enforce clear ownership rules (terrain/static things in static, pawns/interaction in dynamic) and add assertions in fixture v2 path.
- Fixture schema churn:
  - version fixtures (`schema_version`) and keep v2 schema intentionally narrow.

## Definition of Done (v2)

- v2 scene content is loaded from RON fixtures (not sprawling hardcoded scene construction).
- User can hover/select pawn and issue click-to-move.
- Selected pawn follows visible path intent reliably.
- Movement and layering remain stable across repeated runs.
- Structural checks and tolerant visual checks are in place.
