# Lighting, Normals, And Shadows Investigation

## Current State

`plans/BACKLOG.md` currently captures the renderer gap as "Lighting /
shadows. All sprites flat-tinted." That is accurate for `HEAD`:

- `src/shader.wgsl` samples the sprite texture and returns `base * tint`.
- `src/renderer.rs` has one main sprite pass plus terrain-edge and water
  pipelines. It has no lighting overlay, shadow overlay, or normal-map
  pipeline.
- `src/defs.rs::GraphicData` does not parse `shadowData`, and `ThingDef` does
  not parse `blockLight`, `holdsRoof`, `castEdgeShadows`, or
  `staticSunShadowHeight`.
- `src/fixtures/schema.rs` has no roof, fog, glow, time-of-day, or light-source
  fields, so fixture scenes cannot currently express the v0 lighting fixture in
  `docs/plans/parity_fixtures_v0.md`.

Core RimWorld XML uses the missing fields broadly enough that they are not edge
cases. In the local Core defs snapshot, `<shadowData>` appears 312 times,
`<staticSunShadowHeight>` 35 times, `<castEdgeShadows>` 50 times,
`<blockLight>` 13 times, and glower radius/color pairs 25 times. Doors are a
representative blocker: `Buildings_Structure.xml` sets `holdsRoof`,
`staticSunShadowHeight`, and `blockLight` on the door base.

## RimWorld Lighting Model

RimWorld's map lighting is not per-sprite normal lighting. It is an overlay mesh
fed by `GlowGrid` and sky state.

Frame order matters. `Verse.Map.MapUpdate` updates `skyManager`, then
`glowGrid`, then draws map meshes, then dynamic things; this is already traced
in `docs/research/frame-pipeline.md`.

`Verse.SkyManager.SkyManagerUpdate` sets global material color for
`MatBases.LightOverlay`, updates fog color, computes the sun shadow vector, and
pushes water sun/moon vectors. `RimWorld.GenCelestial.GetLightSourceInfo`
derives shadow/sun/moon vectors and intensities from day percent and celestial
glow. The water work deliberately deferred this sun/moon path.

`Verse.SectionLayer_LightingOverlay` bakes a 17x17-section overlay. It creates
corner vertices plus one center vertex per cell, then colors vertices from the
average of nearby `GlowGrid.VisualGlowAt` samples. Roofed cells force a minimum
alpha of 100 when the supporting building does not hold the roof. Light blockers
are skipped when averaging glow.

Implication for this repo: implement lighting as a separate post-terrain overlay
using a cell grid, not by modifying every sprite shader first.

## RimWorld Shadow Model

There are three relevant shadow paths.

1. Static sun shadows: `Verse.SectionLayer_SunShadows` emits geometry for
   buildings with `staticSunShadowHeight > 0`. The shader/material uses
   `MapSunLightDirection`, which `SkyManager` sets from `GenCelestial`.
2. Edge shadows: `Verse.SectionLayer_EdgeShadows` emits local darkening around
   buildings with `castEdgeShadows`. It uses fixed 0.45-cell insets and vertex
   colors from lit white to shadow value 195.
3. Graphic shadows: `Verse.Graphic.ShadowGraphic` constructs
   `Graphic_Shadow` from `GraphicData.shadowData`. `Printer_Shadow` and
   `MeshMakerShadows` build a soft rectangular/edge-fade mesh from
   `ShadowData.volume` and `ShadowData.offset`. Dynamic pawns also draw shadows
   through `PawnRenderer.DrawShadowInternal`, using race special shadow data or
   the body graphic shadow.

Implication for this repo: shadow support should start with def parsing and
static overlay geometry, then add `shadowData` for plants/objects/pawns. Dynamic
shadow-only culling can come after the first visible pass is correct.

## Normals

I did not find a RimWorld map-render normal-map path in the decompile or Core
defs relevant to this renderer. Searches for normal-map/bump-map fields in Core
defs found only prose and unrelated "normal" words. The decompiled shader list
is centered on cutout, transparent, terrain, water, lighting overlay, and shadow
materials; the map renderer gets depth from altitude layers and soft visual
volume from shadow meshes, not sprite normal maps.

Implication for this repo: defer normals unless a modded asset format introduces
normal maps. Treat "normals" here as a non-goal for v0 parity, and spend effort
on overlay lighting and shadow geometry instead.

## Recommended Implementation Order

1. Parse render-relevant fields:
   `ThingDef.blockLight`, `ThingDef.holdsRoof`, `ThingDef.castEdgeShadows`,
   `ThingDef.staticSunShadowHeight`, `GraphicData.shadowData`, race
   `specialShadowData`, and glower comp props.
2. Extend fixtures with deterministic render-state fields:
   roofed cells, fogged cells, day percent or fixed sky glow, and optional
   artificial glow sources.
3. Add a lighting overlay pipeline:
   build a per-section/per-map overlay mesh with corner and center vertices,
   copy the `SectionLayer_LightingOverlay` averaging rules, and draw it after
   world sprites at the equivalent `LightingOverlay` altitude/pass point.
4. Add static sun and edge shadow overlays:
   generate sun-shadow quads from `staticSunShadowHeight`, edge-shadow geometry
   from `castEdgeShadows`, and share a uniform for sun/shadow vector and
   strength.
5. Add `shadowData` graphic shadows:
   emit soft fade meshes from `ShadowData.volume`/`offset` for map-mesh things,
   then dynamic/pawn shadows once culling and draw order need them.
6. Wire Core glowers:
   parse `CompProperties_Glower` radius/color, create a deterministic glow-grid
   approximation, and add parity fixtures around walls/doors/roofed rooms.

## Open Questions

- Whether to first build a simple fixture-only glow grid or a reusable runtime
  `GlowGrid` equivalent. The latter is more work, but avoids rewriting when
  simulation arrives.
- Whether lighting and shadow overlays should be represented as new renderer
  input types or folded into existing sprite/edge batches. New input types look
  cleaner because both systems need custom vertex color geometry.
- How closely to match Unity shadow shaders. Geometry and vertex colors are
  visible from decompiled C#, but exact material shader math is binary. A
  visually equivalent WGSL implementation should be enough for v0.
