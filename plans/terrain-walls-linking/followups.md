# Followups — Terrain Transitions + Wall Linking

Post-implementation notes for the feature shipped in branch
`feat/terrain-walls-linking`. Things that are intentionally deferred,
open judgment calls, and known rough edges worth revisiting when the
surrounding systems are built out.

## Deferred by original plan (Non-Goals from `plan.md` §2)

- **`LinkDrawerType::Transmitter` / `TransmitterOverlay`** — needs a
  power-net graph we don't have yet. Power conduits currently render as
  `Basic` via the fallback in `linking_sprites::effective_link_type`.
- **`LinkDrawerType::Asymmetric` (fences)** — uses a second flag set on
  `GraphicData` we haven't added. Same fallback path as above.
- **`Graphic_Appearances` stuff variants (Smooth / Bricks / Planks)** —
  no stuff system; `linked_atlas_path` hardcodes `_Atlas_Bricks` for
  `Graphic_Appearances`. When stuff is introduced, the atlas basename
  must be picked from the stuff's `stuffProps.appearance`. See
  `commands/linking_sprites.rs::linked_atlas_path` for the insertion
  point.
- **Water terrain rendering (base pass + depth pass)** —
  `TerrainEdgeType::Water` currently falls through the FadeRough edge
  shader branch, but the **base cell** for water terrains is also wrong:
  RimWorld draws water as two passes and we draw it as one. Base pass
  uses `ShaderDatabase.TerrainWater` with `texturePath` pointing at a
  *gradient ramp* (e.g. `Terrain/Surfaces/WaterShallowRamp`, not a
  tileable water tile) plus an `_AlphaAddTex` input from
  `TexGame.AlphaAddTex`, injected in `Verse/TerrainDef.cs:434-437`. A
  second pass via `SectionLayer_Watergen` (`Verse/SectionLayer_Watergen.cs:6-34`)
  draws `terrain.def.waterDepthMaterial` — built from
  `waterDepthShader = "Map/WaterDepth"` — into a separate
  `SubcameraDefOf.WaterDepth` buffer that composites underneath. Without
  both passes, the ramp texture reads directly to screen as a muddy
  brown gradient. `fixtures/v2/terrain_mix.ron` swapped its WaterShallow
  pocket for Ice until the water pipeline lands. Animated wave
  distortion is the next step after the two-pass basics work.
- **Door-linking via `asymmetricLink.linkToDoors`** — doors aren't
  drawn yet.
- **Section batching for edge overlays** — 9-vertex fan emission landed
  in `feat/terrain-edges-9vert`; batching the per-cell fans into
  section-sized vertex buffers is still deferred. The current emission
  is O(map_cells × neighbor_defs). At our ≤24×16 fixture scale it's
  fine. The scale ceiling is ~100 cells per side before per-cell fan
  count becomes the long pole.
- **`Custom1..10` link flags** — unused in Core; add on demand.

## Adjacent bugs noticed during this work

- **`TerrainDef` XML inheritance is not resolved.** `parse_terrain_def`
  drops any def that has no direct `texturePath` child, so terrains that
  inherit it from an `Abstract="True"` parent via `ParentName` are
  silently missing at runtime. In vanilla Core this drops **WaterDeep**,
  **WaterMovingChestDeep**, and a handful of other variants (only 29 of
  ~50+ terrain defs load). `terrain_mix.ron` falls back to
  `WaterShallow` for its pond core because of this. Fix is to build a
  two-pass loader that resolves `ParentName` chains before finalizing
  defs — same bug likely exists for `ThingDef` inheritance. Scope it
  into its own commit; it bleeds beyond this feature.

## Known rough edges

- **Hard-coded `CORNER_FILL_UV_RECT = (0.5, 0.6)`**. Matches RimWorld's
  `Graphic_LinkedCornerFiller.ShiftUp` sample point and works for
  Wall_Atlas_Bricks / Rock_Atlas. If a future atlas has a different
  solid-body region, move this to a per-`ThingDef` override.
- **Noise seed step is a single const (`EDGE_NOISE_STEP = 0.31`)**. Fine
  for the current visual target; pick a different small irrational if
  tiling becomes visible at larger map sizes.
- **`edge_texture_path` on `TerrainDef` is parsed but unused**. The
  emission currently takes the neighbor's base `texture_path`. RimWorld
  uses the edge texture for a few terrains (notably water variants with
  dedicated shore tiles). Wire up in `compute_terrain_edge_contributions`
  when a motivating terrain shows up.
- **`thing_grid` staleness if v2 runtime ever moves things**. Today the
  grid is built once in `world_from_fixture` and never updated. v2
  doesn't move things, but if that changes, the grid must be rebuilt or
  incrementally maintained — the wall-link lookup silently reads stale
  data otherwise. Add a debug-assert tied to a thing-move API when one
  exists.

## Decisions deferred until motivating cases show up

- **Stuff-variant atlas selection** — above.
- **Per-def corner-filler UV** — above.
- **Animated water / distortion** — above.
- **Screenshot-diff test harness**. No automated visual regression
  today; fixtures are for human inspection. If visual regressions start
  slipping through, plug into a pixel-diff tool at that point rather
  than pre-building the infra.

## Reference screenshots

Screenshots live under `plans/terrain-walls-linking/reference/` and are
captured manually via:

```
cargo run -- fixture fixtures/v2/walls_patterns.ron \
    --screenshot plans/terrain-walls-linking/reference/walls_patterns.png \
    --no-window

cargo run -- fixture fixtures/v2/terrain_mix.ron \
    --screenshot plans/terrain-walls-linking/reference/terrain_mix.png \
    --no-window

cargo run -- fixture fixtures/v2/mixed_things_pawns.ron \
    --screenshot plans/terrain-walls-linking/reference/mixed_things_pawns.png \
    --no-window
```

Regenerate when touching the renderer, the edge shader, or any fixture
listed above. Commit the regenerated PNG alongside the code change.
