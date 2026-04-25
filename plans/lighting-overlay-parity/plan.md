# Lighting Overlay Parity Plan

## Status

This is the successor to `plans/archive/lighting-normals-shadows/`. The first
lighting/shadow foundation is already landed on `main`: parsed def fields,
fixture render state, colored overlay rendering, lighting overlays, static
shadows, directional `GraphicData.shadowData` shadows, and parsed thing glower
brightness.

The goal of this worktree is not one isolated commit. The goal is to keep
improving the overlay-based RimWorld lighting model while making each commit
land in a shape that helps the later commits. New helpers should therefore be
small, shared, deterministic, and named around RimWorld concepts rather than
around a single fixture.

Completed in this worktree so far:

- Planned the successor lighting overlay parity workstream.
- Added deterministic fixture sky/shadow state derivation.
- Kept derived shadows darkening-only under the current source-over overlay
  blend path while retaining RimWorld-style material shadow color.
- Unified static overlay construction behind an error-returning entry point.
- Introduced a shared fixture `GlowGrid` boundary for artificial
  `VisualGlowAt`-style glow, separate from sky brightness. The first version is
  intentionally scalar brightness only; later color-parity work should widen
  this toward color/RGBA samples.

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

1. **Deterministic sky/shadow state.** Move shadow vector, shadow strength, and
   default shadow color decisions behind one helper. Explicit fixture overrides
   still win, but fixtures with only `render.day_percent` should get stable,
   RimWorld-shaped shadows.
2. **Shared glow model.** Replace ad hoc per-overlay glower brightness with a
   reusable fixture/runtime glow grid representation that represents
   `GlowGrid.VisualGlowAt`, not sky lighting. Keep the first version
   deterministic and small enough to verify with unit tests. It may remain a
   scalar brightness grid for now, but this is a temporary approximation:
   RimWorld's `VisualGlowAt` is color-valued, so later lighting-color parity
   should make artificial glow color a first-class grid value.
3. **Blocker-aware propagation.** Teach the glow model about `blockLight`,
   doors/walls, and blocker cells before expanding visual tuning. This makes
   the lighting overlay's corner sampling depend on the same source of truth as
   future runtime glower updates.
4. **Lighting color and overlay parity.** Move beyond black alpha darkness where
   the current simplification prevents matching RimWorld: sky color, shadow
   color, and artificial light color should be represented explicitly even if
   the shader remains simple.
5. **Fog and snow overlays.** The fixture state already carries fog and snow;
   render them as overlay systems after the sky/glow model has a clearer home.
6. **Dynamic shadows.** Add pawn/dynamic thing shadow handling once static
   shadow inputs and ordering are stable enough to reuse.
7. **Visual tuning and regression checks.** Add paired deterministic fixtures or
   generated overlay assertions whenever a visual claim depends on relative
   behavior such as morning versus evening direction.

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
  The helper should return both the RimWorld-style material shadow color and the
  current renderer's source-over overlay emission color. Until a multiply/darken
  shadow pipeline exists, derived overlay emission must keep RGB black and put
  strength in alpha; lerping white into an alpha-blended overlay can brighten
  dark pixels instead of only darkening them.

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
  keeps the RimWorld/SkyManager concept available for a later shadow shader;
  current source-over geometry should use `overlay_shadow_color`.
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
- Add a unit test proving derived source-over overlay color stays darkening-only
  even when material shadow color is a partial white-to-shadow lerp.
- Use paired fixture coverage or direct overlay vertex/color assertions for
  visual-relative behavior. One screenshot is not enough to prove that shadows
  changed because only `render.day_percent` changed.
- Before committing, run:
  - `cargo fmt -- --check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`

## Deferred Questions

- Whether fixture render state should grow explicit latitude/day-of-year fields
  before attempting closer `GenCelestial` parity.
- Whether colored overlay rendering should support separate blend modes for
  lighting, fog, snow, and shadows instead of sharing one alpha-blend path.
- Whether dynamic pawn shadows should use the same static overlay builder with
  per-frame inputs or a separate dynamic overlay buffer.
