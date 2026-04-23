# Backlog

Deferred work that doesn't warrant its own plan folder yet. Add new items here; promote to `plans/<feature>/` when picking up.

## Open

### Correctness / bugs

- **`TerrainDef` XML inheritance resolution.** `parse_terrain_def` drops defs without a direct `texturePath`, silently losing `WaterDeep`, `WaterMovingChestDeep`, and ~20 other variants that inherit via `ParentName`. Only 29 of ~50+ terrain defs load. Likely same bug for `ThingDef`. Being fixed on `feat/water-rendering` (commit `e358048`).
- **`thing_grid` staleness if v2 ever moves things.** Built once in `world_from_fixture`, never updated. Wall-link lookup silently reads stale data if this assumption breaks. Add a debug-assert tied to a thing-move API when one exists.

### Rendering

- **Water terrain base pass + depth pass.** Currently water cells fall through the single-pass FadeRough branch. RimWorld renders water as two passes: base gradient ramp (`ShaderDatabase.TerrainWater` + `_AlphaAddTex`) and a `SectionLayer_Watergen` depth composite. Active on `feat/water-rendering`.
- **Animated water / distortion.** Follows the two-pass basics.
- **Door linking + rendering.** Doors aren't drawn.
- **Section batching for edge overlays.** 9-vertex fan emission is O(cells × neighbor_defs). Fine at ≤24×16 fixtures; batch into section-sized vertex buffers before ~100 cells/side.
- **`edge_texture_path` on `TerrainDef` parsed but unused.** Emission takes the neighbor's base `texture_path`. Wire up in `compute_terrain_edge_contributions` when a motivating water shore terrain appears.
- **Hard-coded `CORNER_FILL_UV_RECT = (0.5, 0.6)`.** Works for Wall_Atlas_Bricks / Rock_Atlas. Move to a per-`ThingDef` override if another atlas needs a different sample.

### Linking / stuff system

- **`LinkDrawerType::Transmitter` / `TransmitterOverlay`.** Needs a power-net graph. Power conduits render as `Basic` via fallback in `linking_sprites::effective_link_type`.
- **`LinkDrawerType::Asymmetric` (fences).** Needs second flag set on `GraphicData`.
- **`Graphic_Appearances` stuff variants (Smooth / Bricks / Planks).** No stuff system yet; `linked_atlas_path` hardcodes `_Atlas_Bricks` for `Graphic_Appearances`. When stuff lands, pick atlas basename from `stuffProps.appearance`. Insertion point: `commands/linking_sprites.rs::linked_atlas_path`.
- **`Custom1..10` link flags.** Unused in Core; add on demand.

### Simulation / systems

- **Autonomous pawn AI.** Pawns idle until right-click. Job queue, needs, mood not yet in scope.
- **Lighting / shadows.** All sprites flat-tinted.
- **Save / load runtime state.** Can load RimWorld XML + Unity assets; no runtime serialization.

### Test infra

- **Screenshot-diff test harness.** No automated visual regression today; fixtures are for human inspection. Plug in a pixel-diff tool if regressions start slipping through.

## Historical context

Longer-form retrospective notes live alongside the shipped feature:

- `plans/archive/terrain-walls-linking/followups.md` — full context on deferred wall/terrain linking items (many summarized above).
