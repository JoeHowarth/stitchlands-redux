# Lighting Overlay Parity Plan

## Status

This is the successor to `plans/archive/lighting-normals-shadows/`. The first
lighting/shadow foundation is already landed on `main`: parsed def fields,
fixture render state, colored overlay rendering, lighting overlays, static
shadows, directional `GraphicData.shadowData` shadows, and parsed thing glower
brightness.

The goal of this feature branch is not one isolated commit. The goal is to keep
improving the overlay-based RimWorld lighting model while making each commit
land in a shape that helps the later commits. New helpers should therefore be
small, shared, deterministic, and named around RimWorld concepts rather than
around a single fixture.

Completed on this branch so far:

- Planned the successor lighting overlay parity workstream.
- Added deterministic fixture sky/shadow state derivation.
- Initially kept derived shadows darkening-only under the source-over overlay
  blend path while retaining RimWorld-style material shadow color.
- Unified static overlay construction behind an error-returning entry point.
- Introduced a shared fixture `GlowGrid` boundary for artificial
  `VisualGlowAt`-style glow, separate from sky brightness.
- Added blocker-aware `GlowGrid` propagation with fixed cardinal/diagonal
  attenuation costs and internal color-carrying samples.
- Split lighting overlay sampling into explicit sky color, artificial glow
  color, combined brightness, and current source-over darkness emission.
- Added renderer overlay blend modes and moved shadow overlays onto a multiply
  path so derived material shadow colors can darken without source-over
  brightening artifacts.

The current position is the environmental overlay milestone. Known missing
scope is scheduled by dependency below rather than deferred indefinitely: fog
and snow overlays come next, then dynamic overlay work.

## Reference Model

RimWorld's map lighting is a set of overlay systems layered around sprite
drawing. `Map.MapUpdate` updates sky state, then glow state, then map mesh
layers, then dynamic things. The relevant decompiled anchors are:

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
- `plans/archive/lighting-normals-shadows/context.md`

`SkyManager.SkyManagerUpdate` gets the default shadow vector from
`GenCelestial.GetLightSourceInfo(map, GenCelestial.LightType.Shadow).vector`
unless a sky-affecting event or thing provides an override. It also computes the
sun shadow material color by lerping from white to the current sky shadow color
using `GenCelestial.CurShadowStrength(map)`.

`GlowGrid` is the durable runtime source for artificial light. Buildings and
things register glowers, blockers affect propagation, and
`SectionLayer_LightingOverlay` samples `GlowGrid.VisualGlowAt` at cell corners.
The current repo only approximates this with direct radial brightness and a
corner-time `blockLight` skip.

Keep the sky model and glow model separate. RimWorld's `GlowGrid.VisualGlowAt`
is accumulated artificial/map glow, while sky color, sky glow, fog color, and
shadow material color come from `SkyManager` and material state. The current
`src/commands/lighting_overlay.rs` mixes sky brightness into per-cell darkness;
the shared glow work should make that split explicit instead of preserving the
conflation in a more permanent API.

## Workstream Arc

1. **Deterministic sky/shadow state: complete.** Shadow vector, shadow strength,
   and default shadow color decisions now live behind a shared helper. Explicit
   fixture overrides still win, while fixtures with only `render.day_percent`
   get stable, RimWorld-shaped shadows.
2. **Shared glow model boundary: complete.** Artificial glow now has a reusable
   fixture/runtime `GlowGrid` boundary that represents `GlowGrid.VisualGlowAt`,
   not sky lighting.
3. **Blocker-aware propagation: complete.** Direct radial glow sampling has been
   replaced with flood-fill attenuation seeded by fixture glow sources and
   parsed ThingDef glowers. `ThingDef.blockLight` is the source of truth for
   blockers, not pathing movement flags. The model carries both propagated
   intensity and source color, while the current lighting overlay still emits
   black-alpha until the rendering blend model is ready for color.
4. **Lighting color and overlay parity: complete.** Artificial glow color, sky
   color, combined brightness, and source-over darkness emission are now
   represented explicitly in the lighting overlay model. The renderer still
   displays lighting through scalar darkness until additive/tint lighting gets
   a dedicated blend path.
5. **Renderer blend modes: complete for shadows.** Colored overlays now carry a
   blend mode, and shadow overlays use a multiply/darken path. This removes the
   source-over limitation that forced derived shadows to use black RGB plus
   alpha.
6. **Fog and snow overlays: next.** Render fixture fog and snow grids after sky,
   glow, color, and blend-mode semantics are clear enough to avoid baking in
   temporary darkness-overlay behavior.
7. **Dynamic lighting and shadows.** Add dynamic glower updates and pawn/dynamic
   thing shadows after static/environment overlay paths stabilize, reusing the
   same glow, shadow-state, and renderer mechanisms.
8. **Visual tuning and regression checks: ongoing.** Add paired deterministic
   fixtures or generated overlay assertions whenever a visual claim depends on
   relative behavior such as morning versus evening direction.

## Completed Commit: Blocker-Aware GlowGrid Propagation

This implementation commit kept the current `GlowGrid` API but replaced its
direct radial sampling with a deterministic propagation model. This is the last
structural step before lighting color parity: the system now has a single place
where artificial glow is seeded, blocked, attenuated, and later colored.

### Decisions

- Use flood-fill attenuation over the map grid rather than per-cell
  line-of-sight checks. This better matches a cached grid model and gives later
  runtime updates a clear shape.
- Use `ThingDef.blockLight` as the blocker source of truth. Do not infer light
  blocking from `ThingState.blocks_movement`.
- Treat missing glowers as normal zero-glow state. The no-silent-fallback rule
  still applies to required authored sky/shadow inputs, but a map with no glow
  emitters should simply produce zero artificial glow.
- Carry glow color internally in addition to scalar intensity. The current
  renderer should continue consuming scalar `visual_glow_at` so screenshots stay
  stable; color should become visible only in the later lighting color/blend
  milestones.

### Implementation Notes

- Build a per-cell blocker grid from static things whose defs have
  `blockLight`.
- Seed propagation from both `render.glow_sources` and ThingDef glowers.
- Preserve the current overlight behavior near a source where possible.
- Use a fixed first attenuation rule: cardinal steps cost `1.0`, diagonal
  steps cost `sqrt(2)`, and propagation stops once accumulated distance exceeds
  the source radius. A source cell may be a `blockLight` cell: it should still
  receive and emit its own glow, but propagation should not pass through other
  blocker cells after entering them.
- Combine overlapping glow with a deterministic max or stronger-wins rule for
  the first propagation version. Avoid introducing color blending semantics that
  the renderer cannot yet display.
- Keep sky brightness out of `GlowGrid`; lighting overlays may combine sky
  brightness with `GlowGrid::visual_glow_at`, but the grid itself remains
  artificial glow only.

### Verification

- A fixture glow source brightens nearby unobstructed cells.
- A ThingDef glower brightens nearby unobstructed cells through the same path.
- A `blockLight` thing reduces or stops glow behind it.
- A movement-blocking thing without `blockLight` does not block glow.
- Sky inputs still do not register as `GlowGrid` inputs and do not change
  `GlowGrid` samples.
- Existing lighting overlay tests remain stable.

## Completed Commit: Sky-Derived Shadow State

The first implementation commit added the shared helper for
deterministic fixture sky/shadow state. This is intentionally the first step in
the broader arc because later static shadows, dynamic shadows, fog, and water
sun vectors should not each invent their own day-percent interpretation.

### Deterministic Rule

Use this fixture-only approximation for every fixture that wants derived
sky/shadow state:

- Require `render.day_percent`. If a scene needs shadow overlays and does not
  provide either a complete explicit shadow state or `render.day_percent`, the
  build should fail with a clear error instead of falling back silently. The
  existing hard-coded fallback vector should be removed from the shadow overlay
  path as part of this work.
- A complete explicit shadow state is `render.shadow_vector` plus
  `render.shadow_color`. Providing only one of those fields is allowed, but the
  missing field is still derived from `render.day_percent`; if
  `render.day_percent` is absent, that partial override is an error.
- Explicit `render.shadow_vector` overrides only the vector. It does not imply a
  default color or strength.
- Explicit `render.shadow_color` overrides only the final shadow RGBA color. It
  does not imply a default vector.
- With no explicit overrides, derive both vector and final shadow RGBA color
  from `render.day_percent`.
- Clamp `render.day_percent` into `0.0..=1.0` at the derived-state boundary.
  Fixture validation should still reject invalid authored values earlier when
  possible.
- Derive fixture `sun_glow` from day percent with a simple deterministic curve:
  `sun_glow = clamp(1.0 - abs(day_percent - 0.5) * 2.0, 0.0, 1.0)`.
- Treat `sun_glow > 0.6` as the daytime path. This matches RimWorld's
  `GenCelestial.IsDaytime(glow)` threshold while avoiding latitude and season
  inputs that fixtures do not currently model.
- For daytime shadows, use RimWorld's daytime vector shape:
  `t = day_percent`, `x = lerp(-15.0, 15.0, t)`, and
  `z = -1.5 - 2.5 * (x * x / 100.0)`.
- For non-daytime shadows, use RimWorld's moon-style wrapped path:
  `t = if day_percent > 0.5 { inverse_lerp(0.5, 1.0, day_percent) * 0.5 } else { 0.5 + inverse_lerp(0.0, 0.5, day_percent) * 0.5 }`,
  `x = lerp(-15.0, 15.0, t)`, and
  `z = -0.9 - 2.5 * (x * x / 100.0)`.
- Derive `shadow_strength = clamp(abs(sun_glow - 0.6) / 0.15, 0.0, 1.0)`,
  matching RimWorld's `CurShadowStrength` shape but using fixture `sun_glow`.
- If `render.shadow_color` is absent, use a documented default sky shadow color
  and lerp white to that color by `shadow_strength`, matching the shape of
  `SkyManager`'s `Color.Lerp(Color.white, curSky.colors.shadow, strength)`.
  The helper returns both the RimWorld-style material shadow color and the
  renderer overlay emission color. Before multiply blending existed, derived
  overlay emission stayed black-alpha to avoid source-over brightening; after
  the multiply blend commit, derived overlay emission can use the material
  shadow color directly.

This rule is not full RimWorld sky parity. It is a deterministic fixture rule
that preserves the important shape: shadow direction moves with time of day,
day/night chooses the corresponding vector path, and shadow strength is derived
from glow rather than directly from raw `day_percent`.

### Implementation Notes

- Put the helper in a reusable location rather than only in
  `src/commands/shadow_overlay.rs`; later overlay systems should be able to call
  it.
- Return a small value object with at least `shadow_vector`,
  `shadow_strength`, `sun_glow`, `material_shadow_color`,
  `overlay_shadow_color`, and `shadow_alpha_scale`. `material_shadow_color`
  keeps the RimWorld/SkyManager concept available to the multiply shadow path;
  shadow geometry should use `overlay_shadow_color`.
- Keep explicit `render.shadow_vector` as the highest-priority vector override.
- Keep explicit `render.shadow_color` as the highest-priority color override.
- Make missing required inputs an error in the fixture/shadow build path. Do not
  add new silent fallback behavior.
- Update static sun shadows and graphic shadows to use the helper. Edge shadows
  should remain local edge darkening unless there is a specific RimWorld reason
  to tie them to sky state.
- Existing fixtures with explicit vectors should remain stable.

### Verification

- Add unit tests for morning, noon, evening, and night vector paths.
- Add a unit test proving explicit `render.shadow_vector` overrides derived
  day-percent behavior.
- Add unit tests for all input combinations: complete explicit shadow state with
  no `day_percent`, vector-only plus `day_percent`, color-only plus
  `day_percent`, vector-only without `day_percent` error, color-only without
  `day_percent` error, and no shadow state without `day_percent` error when a
  shadow overlay is requested.
- Add a unit test proving shadow strength comes from derived `sun_glow`, not raw
  `day_percent`.
- Add a unit test proving derived overlay color uses the material shadow color
  once shadow overlays render through the multiply path.
- Use paired fixture coverage or direct overlay vertex/color assertions for
  visual-relative behavior. One screenshot is not enough to prove that shadows
  changed because only `render.day_percent` changed.
- Before committing, run:
  - `cargo fmt -- --check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`

## Scheduled Follow-Up Milestones

- **Fog and snow overlays, next.** Render the existing fixture fog
  and snow grids as first-class overlay systems.
- **Dynamic overlays, after static/environment overlays.** Add dynamic glower
  updates and pawn/dynamic thing shadows using the same shared systems.

## Open Design Questions

- Whether fixture render state should grow explicit latitude/day-of-year fields
  before attempting closer `GenCelestial` parity.
- The exact color-combination rule for overlapping artificial glowers once the
  renderer can display colored lighting.
- Whether dynamic pawn shadows should use the static overlay builder with
  per-frame inputs or a separate dynamic overlay buffer.
