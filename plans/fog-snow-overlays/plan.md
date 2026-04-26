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

## Implementation Shape

- Keep fog and snow as separate overlay builders. They may share low-level mesh
  helpers, but the public boundaries should stay named around RimWorld systems.
- Consume existing fixture state from `RenderState::fog` and
  `RenderState::snow_depth`; do not invent unrelated scene inputs.
- Preserve the completed lighting split: fog/snow overlays should compose over
  terrain and lighting/shadow output instead of changing `GlowGrid` or sky
  derivation.
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

