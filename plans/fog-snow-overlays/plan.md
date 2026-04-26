# Fog and Snow Overlay Plan

## Status

Active. This is the next small overlay slice after the shipped lighting,
shadow, glow, blend-mode, and static sun shadow foundation in
`plans/archive/lighting-overlay-parity/`.

The goal is to render authored fixture fog and snow grids as first-class
overlays without reopening the completed lighting plan or mixing in dynamic
lighting scope.

## Reference Model

Start with the decompiled RimWorld sources before implementation:

- `Verse/SectionLayer_FogOfWar.cs`
- `Verse/SectionLayer_Snow.cs`
- `Verse/SnowGrid.cs`
- `Verse/FogGrid.cs`

Use `~/rimworld-decompiled/MAP/INDEX.md` to find exact file:line anchors before
porting. Record the relevant methods and shader/material expectations in this
plan before writing code.

### Port Notes

- `MAP/graphics-primitives.md:152-154` identifies the relevant render layers:
  `SectionLayer_Snow` uses `MatBases.Snow`, `SectionLayer_FogOfWar` uses
  `MatBases.FogOfWar`, and both are section mesh overlays.
- `Verse/SectionLayerGeometryMaker_Solid.cs:7-61` is the shared topology:
  for each cell it emits 9 vertices in this order: south-west corner, west
  midpoint, north-west corner, north midpoint, north-east corner, east midpoint,
  south-east corner, south midpoint, center. It then emits 8 triangles around
  that center. This is the topology fog and snow should preserve locally.
- `Verse/SectionLayer_FogOfWar.cs:19-116` builds the fog mesh at
  `AltitudeLayer.FogOfWar` with `MatBases.FogOfWar`. A fogged cell sets all
  nine vertex alphas to 255. An unfogged cell starts with all alphas 0, then
  marks edge and corner vertices covered from the 8-neighborhood of fogged
  cells. The section is disabled when every alpha is 0.
- `Verse/SkyManager.cs:22,49-59` owns the fog material color. Vanilla uses base
  fog color `Color32(77, 69, 66, 255)` and, when sky lighting is active,
  multiplies it by the current sky color before assigning `MatBases.FogOfWar`.
  This slice should use the base color unless/ until fixture sky color is needed
  for visible parity.
- `Verse/FogGrid.cs:71-87` exposes fog as a map-sized boolean grid with
  out-of-bounds reads returning not fogged. Local input is already
  `RenderState::fog`.
- `Verse/SectionLayer_Snow.cs:49-117` builds snow with
  `SectionLayerGeometryMaker_Solid` at `AltitudeLayer.Terrain` and
  `MatBases.Snow`. For each cell, it samples `SnowGrid.DepthGrid_Unsafe` at the
  current cell and the `GenAdj.AdjacentCellsAndInsideForUV` offsets, using the
  current cell's depth for out-of-bounds neighbors. It then averages those
  sampled depths through the static `vertexWeights` table and writes opacity to
  the vertex alpha. The section is disabled when all averaged opacities are
  `<= 0.01`.
- `Verse/GenAdj.cs:106-114` gives the snow sample offset order:
  south, south-west, west, north-west, north, north-east, east, south-east,
  inside.
- `Verse/SnowGrid.cs:16-18,100-115,148-155` defines snow depth as a clamped
  `0.0..=1.0` map-sized float grid. Local input is already
  `RenderState::snow_depth`.
- Pollution is encoded into the red color channel by
  `Verse/SectionLayer_Snow.cs:91-104` and uses `Other/SnowPolluted`, but this
  repo has no pollution fixture input yet. Keep pollution out of this slice.

## Implementation Shape

- Keep fog and snow as separate overlay builders. They may share low-level mesh
  helpers, but the public boundaries should stay named around RimWorld systems.
- Consume existing fixture state from `RenderState::fog` and
  `RenderState::snow_depth`; do not invent unrelated scene inputs.
- Preserve the completed lighting split: fog/snow overlays should compose over
  terrain and lighting/shadow output instead of changing `GlowGrid` or sky
  derivation.
- Draw snow after terrain and fog after dynamic sprites unless visual
  verification proves a different local pass is required. This keeps snow
  terrain-like while fog remains a top overlay.
- Add deterministic mesh/color assertions for authored fog and snow grids.
- Add paired fixtures or fixture variants when a visual claim depends on
  relative depth or neighboring cells.

## Acceptance

- A fixture fog grid renders visible fog only on authored fogged cells.
- A fixture snow grid renders visible snow whose strength follows authored
  snow depth.
- Fog and snow behavior is covered by direct overlay assertions and fixture
  renders under `fixtures/renders/`.
- Existing lighting, shadow, glow, water, and wall-linking fixtures still render
  successfully through `cargo run -- render-fixtures`.
- Before committing, run formatting, tests, strict Clippy, and the batch fixture
  render command required by `AGENTS.md`.
