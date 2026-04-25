# Lighting, Normals, And Shadows Context

This note is for future agents picking up implementation after the
investigation pass. Paths under `Verse/`, `RimWorld/`, and `MAP/` are relative
to the frozen decompile at `/Users/jh/rimworld-decompiled/`.

## Fast Reading List

Start with these repo files:

- `plans/lighting-normals-shadows/investigation.md` - high-level conclusion and
  recommended implementation order.
- `src/defs.rs` - current XML parse surface. `ThingDef` and `GraphicData` are
  intentionally narrow today.
- `src/fixtures/schema.rs` - fixture schema currently cannot express roof,
  fog, snow, glow, or fixed sky state.
- `src/world/state.rs` and `src/world/spawn.rs` - runtime fixture state; thing
  grid is static and cell-indexed.
- `src/commands/fixture_cmd.rs` - builds terrain, things, pawns, edge sprites,
  water routing, and viewer launch data from a fixture.
- `src/viewer.rs` - bridges `RenderSprite`/edge sprites into the renderer.
- `src/renderer.rs` - base sprite, terrain-edge, and water passes. There is no
  general colored-overlay mesh path yet.
- `src/shader.wgsl` - base sprite shader is still `texture * tint`.

Then use these decompiled map pages:

- `MAP/INDEX.md` - subsystem index and reading order.
- `MAP/graphics-primitives.md` - `GraphicData`, `Graphic_Shadow`, `MatBases`,
  and `SectionLayer_*` overview.
- `MAP/map-and-world.md` - grid model, including `RoofGrid`, `FogGrid`,
  `SnowGrid`, and `GlowGrid`.
- `MAP/defs-and-loading.md` - render-relevant `ThingDef` fields and XML load
  model.
- `MAP/building-construction.md` - building spawn/despawn side effects for
  `blockLight`, `holdsRoof`, and glower registration.
- `MAP/component-system.md` - `CompGlower` and comp props context.

## Decompiled Source Anchors

Lighting overlay:

- `Verse/SectionLayer_LightingOverlay.cs`
- `Verse/GlowGrid.cs`
- `Verse.Glow/ComputeGlowGridsJob.cs`
- `Verse.Glow/GlowLight.cs`

Sky and sun vectors:

- `Verse/SkyManager.cs`
- `RimWorld/GenCelestial.cs`

Shadow geometry:

- `Verse/SectionLayer_SunShadows.cs`
- `Verse/SectionLayer_EdgeShadows.cs`
- `Verse/Graphic_Shadow.cs`
- `Verse/Printer_Shadow.cs`
- `Verse/MeshMakerShadows.cs`
- `Verse/ShadowData.cs`

Def fields:

- `Verse/ThingDef.cs`
- `Verse/GraphicData.cs`
- `RimWorld/CompProperties_Glower.cs`
- `Verse/CompGlower.cs`

Useful exact fields in `Verse/ThingDef.cs`:

- `castEdgeShadows`
- `staticSunShadowHeight`
- `holdsRoof`
- `blockLight`
- `graphicData`

Useful exact fields in `Verse/GraphicData.cs` and `Verse/ShadowData.cs`:

- `GraphicData.shadowData`
- `ShadowData.volume`
- `ShadowData.offset`

Useful exact fields in `RimWorld/CompProperties_Glower.cs`:

- `overlightRadius`
- `glowRadius`
- `glowColor`
- `colorPickerEnabled`
- `darklightToggle`
- `overrideIsCavePlant`

## Target Behavior Summary

RimWorld map lighting is an overlay mesh, not per-sprite normal lighting.
`Map.MapUpdate` updates `skyManager`, then `glowGrid`, then regenerates/draws
map meshes before dynamic things. This repo should follow the same conceptual
split: keep sprites as sprite draws, and add lighting/shadows as colored overlay
geometry.

`SectionLayer_LightingOverlay` builds a 17x17 section mesh with one vertex at
each cell corner plus one center vertex per cell. Corner vertex colors average
up to four neighboring `GlowGrid.VisualGlowAt` cells. Cells whose edifice has
`blockLight` are skipped from the average. Roofed samples force alpha to at
least `100` if the roof is thick, has no supporting edifice, the edifice does
not `holdsRoof`, or the edifice is a door moveable altitude. Center vertices are
the average of the four surrounding corner vertices, with a similar roofed-cell
alpha floor.

`SkyManager.SkyManagerUpdate` sets the light-overlay material color from the
current sky, computes shadow color, and sets the global sun-shadow vector from
`GenCelestial.GetLightSourceInfo(map, Shadow)`. For deterministic fixtures,
prefer fixed fixture fields such as `day_percent` or direct sky/shadow values
before trying to reproduce the whole weather/game-condition sky stack.

`SectionLayer_SunShadows` emits geometry for buildings whose
`staticSunShadowHeight > 0`. It places base cell quads with low alpha, then adds
edge vertices whose alpha is `255 * staticSunShadowHeight` where neighboring
buildings are absent or lower. The actual cast direction comes from the shader
using the global sun vector; in WGSL this likely becomes either a uniform-driven
vertex offset or equivalent shadow fragment math.

`SectionLayer_EdgeShadows` emits local darkening around buildings with
`castEdgeShadows`. Constants worth preserving are `InDist = 0.45` and
`ShadowBrightness = 195`; vertex colors fade from `195` to `255`.

`Graphic_Shadow` uses `GraphicData.shadowData` to create a `Graphic_Shadow`.
Printed/static shadows call `Printer_Shadow.PrintShadow`; dynamic shadows draw
the mesh directly. `Printer_Shadow` and `MeshMakerShadows` create a soft
rectangular fade mesh using `ShadowData.volume` and `ShadowData.offset`, with
shadow strength packed into vertex alpha from `volume.y`.

Normals should remain a non-goal for v0 parity unless a concrete modded asset
format introduces them. Core RimWorld map rendering gets volume from altitude,
lighting overlays, and shadow geometry rather than normal maps.

## Repo Gaps

`src/defs.rs` currently omits every lighting/shadow field needed for the first
renderer pass:

- `ThingDef.blockLight`
- `ThingDef.holdsRoof`
- `ThingDef.castEdgeShadows`
- `ThingDef.staticSunShadowHeight`
- `GraphicData.shadowData`
- glower comp props

The current `RawThingDef` inheritance resolver only carries `graphicData`.
Adding non-graphic fields should extend the same raw/merge/finalize pattern and
update the stale comment that says only `defName` and `graphicData` are
resolved.

`src/fixtures/schema.rs` has no way to express the render state needed by
`docs/plans/parity_fixtures_v0.md` fixture `F01_terrain_fog_snow_light`:

- roofed cells
- fogged cells
- snow depth
- fixed sky glow or day percent
- artificial glow sources

`src/renderer.rs` has no reusable colored-geometry pipeline. Terrain edge fans
are close in shape but are texture/noise driven and should not be overloaded for
lighting and shadows. Add a separate overlay geometry input type when renderer
work starts.

## Recommended Next Commits

First commit: parse the def fields without renderer changes.

- Add `ShadowData { volume: Vec3, offset: Vec3 }`.
- Add `GraphicData.shadow_data: Option<ShadowData>`.
- Add `ThingDef.block_light`, `holds_roof`, `cast_edge_shadows`, and
  `static_sun_shadow_height`.
- Add a minimal glower representation, probably
  `GlowerProps { glow_radius, glow_color, overlight_radius }`, parsed from comp
  nodes whose `Class` is `CompProperties_Glower` or whose comp class resolves to
  glower in Core-style XML.
- Carry all of the above through `ParentName` inheritance.
- Add unit tests for direct parse and inherited parse.

Second commit: extend fixtures/runtime state.

- Add roof/fog/snow/light fixture fields with defaults that preserve existing
  fixtures.
- Thread those fields into `WorldState` or a small render-state object.
- Add validation for cell bounds and map-sized arrays.

Third commit: renderer overlay foundation.

- Add a colored mesh pipeline with per-vertex position and color.
- Add a `set_static_overlays` or equivalent API.
- Draw overlays at explicit pass points instead of sorting them with sprites.

Fourth commit: lighting overlay fixture implementation.

- Build the RimWorld-style corner/center lighting mesh from fixture glow,
  blockers, and roofs.
- Add deterministic screenshot output for a small roof/blocker/glow scene.

Fifth commit: static and edge shadows.

- Generate sun-shadow and edge-shadow geometry from parsed `ThingDef` fields.
- Add fixture coverage around doors/walls and shadow direction.

Sixth commit: `shadowData` graphic shadows.

- Emit static soft fade meshes for map-mesh things with `shadowData`.
- Defer dynamic/pawn shadow culling until the first visible static pass is
  correct.

## Verification Notes

For the first def-parse commit, targeted Rust tests in `src/defs.rs` are enough
during iteration. Before committing, follow repo policy and run formatting,
tests, and lint. Do not add local Clippy allowances.

For renderer commits, use deterministic screenshot output and inspect the
generated image directly. The useful fixture target is `F01` from
`docs/plans/parity_fixtures_v0.md`, but it needs schema support first.
