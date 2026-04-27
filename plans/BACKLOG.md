# Backlog

Deferred work that doesn't warrant its own plan folder yet. Add new items here; promote to `plans/<feature>/` when picking up.

## Open

### Correctness / bugs

- **Non-graphic `ThingDef` XML inheritance audit.** Render-facing `graphicData` inheritance is resolved, but other inherited fields still need a pass before simulation systems lean on them.
- **`TerrainDef` XML inheritance resolution.** `parse_terrain_def` drops defs without a direct `texturePath`, silently losing `WaterDeep`, `WaterMovingChestDeep`, and ~20 other variants that inherit via `ParentName`. Only 29 of ~50+ terrain defs load. Likely same bug for `ThingDef`. Being fixed on `feat/water-rendering` (commit `e358048`).
- **`thing_grid` staleness if v2 ever moves things.** Built once in `world_from_fixture`, never updated. Wall-link lookup silently reads stale data if this assumption breaks. Add a debug-assert tied to a thing-move API when one exists.

### Rendering

- **Water terrain base pass + depth pass.** Currently water cells fall through the single-pass FadeRough branch. RimWorld renders water as two passes: base gradient ramp (`ShaderDatabase.TerrainWater` + `_AlphaAddTex`) and a `SectionLayer_Watergen` depth composite. Active on `feat/water-rendering`.
- **Animated water / distortion.** Follows the two-pass basics.
- **Door linking + rendering.** Doors aren't drawn.
- **Section batching for edge overlays.** 9-vertex fan emission is O(cells × neighbor_defs). Fine at ≤24×16 fixtures; batch into section-sized vertex buffers before ~100 cells/side.
- **`edge_texture_path` on `TerrainDef` parsed but unused.** Emission takes the neighbor's base `texture_path`. Wire up in `compute_terrain_edge_contributions` when a motivating water shore terrain appears.
- **Hard-coded `CORNER_FILL_UV_RECT = (0.5, 0.6)`.** Works for Wall_Atlas_Bricks / Rock_Atlas. Move to a per-`ThingDef` override if another atlas needs a different sample.
- **`LightOverlay` blend mode parity bug.** RimWorld's `lighting/lightoverlay.shader` uses `Blend DstColor SrcColor` (multiplicative tint where channel values >1 brighten, <1 darken). Today's lighting routes through `colored_overlay.wgsl` with alpha blend, so the math is wrong. Cheap fix once `plans/renderer-engine-boundary/` Commit 1 lands `PipelineSet` (add a dedicated `light_overlay` pipeline with the correct blend state). Cross-check `~/rimworld-shader-extract/assetripper-1.3.5-decompile-export/` for authoritative blend; re-baseline affected fixture renders in the same commit.
- **`SunShadowFade` pipeline missing.** RimWorld ships separate `lighting/sunshadow.shader` and `lighting/sunshadowfade.shader`; we have only the former. Add a second pipeline with the fade variant's blend state. Same trigger as above — depends on `PipelineSet` being in.
- **`MaterialKind` subkinds.** After `plans/renderer-engine-boundary/` Commit 2 introduces `scene::MaterialKind` (12 coarse variants), split when a fixture or parity bug needs the distinction: `CutoutPlant` (sway input), `CutoutSkin` / `CutoutHair` (separate tinting), `TransparentPostLight`, `TerrainFadeRough`. Reference: `~/rimworld-shader-extract/INDEX.md`.

### Refactor / structure

- **`commands/fixture_cmd.rs` cleanup beyond `build_scene` extraction.** After `plans/renderer-engine-boundary/` Commit 3 ships, the command still owns RON loading, def resolution, runtime bootstrap, screenshot output, and launch-spec assembly in 700+ LOC. Separate plan; not part of the renderer boundary work.
- **Static-vs-live `build_scene` split.** Trigger: per-tick rebuild cost shows up in a profile at realistic map sizes. Solution: split into `build_static_scene` + `build_live_frame`; per-overlay functions already partition cleanly. May overlap with dirty tracking (rebuilding only changed regions) — pick the simpler answer at trigger time.

### Linking / stuff system

- **`LinkDrawerType::Transmitter` / `TransmitterOverlay`.** Needs a power-net graph. Power conduits render as `Basic` via fallback in `linking_sprites::effective_link_type`.
- **`LinkDrawerType::Asymmetric` (fences).** Needs second flag set on `GraphicData`.
- **`Graphic_Appearances` stuff variants (Smooth / Bricks / Planks).** No stuff system yet; `linked_atlas_path` hardcodes `_Atlas_Bricks` for `Graphic_Appearances`. When stuff lands, pick atlas basename from `stuffProps.appearance`. Insertion point: `commands/linking_sprites.rs::linked_atlas_path`.

### Simulation / systems

- **Autonomous pawn AI.** Pawns idle until right-click. Job queue, needs, mood not yet in scope.
- **Dynamic lighting / pawn shadow follow-ups.** Overlay lighting, derived sky/shadow vectors, static shadows, graphic shadows, glower brightness, and blocker-aware glow propagation are landed. Fog/snow overlays are active in `plans/fog-snow-overlays/`; dynamic glower updates and pawn/dynamic thing shadows remain deferred.
- **Save / load runtime state.** Can load RimWorld XML + Unity assets; no runtime serialization.

### Test infra

- **Screenshot-diff test harness.** No automated visual regression today; fixtures are for human inspection. Plug in a pixel-diff tool if regressions start slipping through.

## Historical context

Longer-form retrospective notes live alongside the shipped feature:

- `plans/archive/terrain-walls-linking/followups.md` — full context on deferred wall/terrain linking items (many summarized above).
- `plans/archive/lighting-overlay-parity/plan.md` — shipped lighting, glow, shadow, blend-mode, and static sun shadow foundation.
