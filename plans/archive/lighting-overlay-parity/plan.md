# Lighting Overlay Parity Plan

## Status

Shipped on `main`.

This plan replaced the earlier `plans/archive/lighting-normals-shadows/`
investigation with an overlay-first RimWorld lighting and shadow foundation.
The active follow-up work has moved to `plans/fog-snow-overlays/`; dynamic
lighting and pawn shadows remain backlog-level until the static/environment
overlay paths settle further.

## Landed Scope

- Deterministic fixture sky/shadow state derivation from `render.day_percent`,
  with explicit partial override rules for `render.shadow_vector` and
  `render.shadow_color`.
- A shared fixture/runtime `GlowGrid` boundary for artificial
  `VisualGlowAt`-style glow, separate from sky brightness.
- Blocker-aware glow propagation using fixed cardinal/diagonal attenuation and
  `ThingDef.blockLight` as the blocker source of truth.
- Lighting overlay samples split into sky color, artificial glow color,
  combined brightness, and current renderer darkness emission.
- Colored overlay blend-mode plumbing, including a multiply/darken path for
  shadow overlays.
- Derived material shadow colors for graphic and static shadows.
- Static sun shadow rendering moved to a `SectionLayer_SunShadows`-shaped
  boundary: Rust emits unprojected section-style mesh topology and
  `src/sun_shadow.wgsl` projects cast vertices from shader state.
- Regression fixtures for single-wall static sun shadows, opposite cast
  direction, and top-right cast direction.
- `cargo run -- render-fixtures` for batch fixture screenshot generation into
  `fixtures/renders/`.

## Important References

RimWorld anchors:

- `Verse/SkyManager.cs`
- `RimWorld/GenCelestial.cs`
- `Verse/GlowGrid.cs`
- `Verse.Glow/ComputeGlowGridsJob.cs`
- `Verse.Glow/GlowLight.cs`
- `Verse/SectionLayer_LightingOverlay.cs`
- `Verse/SectionLayer_SunShadows.cs`
- `Verse/SectionLayer_EdgeShadows.cs`
- `Verse/Graphic_Shadow.cs`
- `Verse/Printer_Shadow.cs`
- `Verse/MeshMakerShadows.cs`

Local implementation anchors:

- `src/commands/sky_shadow.rs`
- `src/commands/glow_grid.rs`
- `src/commands/lighting_overlay.rs`
- `src/commands/shadow_overlay.rs`
- `src/renderer.rs`
- `src/colored_overlay_multiply.wgsl`
- `src/sun_shadow.wgsl`
- `fixtures/v2/lighting_overlay.ron`
- `fixtures/v2/glower_lighting.ron`
- `fixtures/v2/shadow_data.ron`
- `fixtures/v2/single_wall_static_shadow.ron`
- `fixtures/v2/single_wall_static_shadow_opposite.ron`
- `fixtures/v2/single_wall_static_shadow_top_right.ron`

## Design Decisions To Preserve

- Keep sky lighting and artificial glow separate. `GlowGrid::visual_glow_at`
  represents artificial/map glow, while sky color, sky glow, fog color, and
  shadow material color come from sky/render state.
- Missing authored sky/shadow inputs should fail clearly when a requested
  shadow overlay needs them; do not add silent fallback vectors.
- Use `ThingDef.blockLight` for glow blockers. Do not infer light blocking from
  movement blocking.
- For static sun shadows, keep mesh construction and cast-vector projection
  separate. CPU-side code owns the section-style topology; the renderer owns
  displacement through `SunShadowParams` and `src/sun_shadow.wgsl`.
- For RimWorld ports, preserve authored inputs, runtime state, mesh topology,
  material colors, shader uniforms, neighbor rules, and silhouette rules before
  adding renderer-specific adapters.

## Residual Risks

- Static sun shadow parity has unit and fixture coverage, but still needs
  recurring zoomed visual inspection until screenshot-diff infrastructure
  exists.
- Lighting color is still displayed through the current scalar darkness
  overlay path; visible colored lighting needs a dedicated later renderer pass.
- Dynamic glower updates and pawn/dynamic thing shadows have not been ported.
- Fog and snow grids are parsed in fixtures but are not first-class overlay
  renderers yet.

