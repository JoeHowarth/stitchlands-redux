#  Followups — Water Rendering

Post-implementation notes for the feature shipped in branch
`feat/water-rendering`. What got deferred, what tuning handles exist,
and what adjacent bugs turned up.

## Deferred by original plan (Non-Goals from `plan.md` §2)

- **River flow (`_WaterOffsetTex`).** `WaterInfo.riverFlowMap`
  (`Verse/WaterInfo.cs:59-91`) encodes a per-cell 2D flow vector,
  uploaded as an `RGFloat` texture, sampled by the moving-water
  shaders. Needs a river-flow map generator first; defer until a
  fixture shows off a real river.
- **Sun/moon specular.** `_WaterCastVectSun` / `_WaterCastVectMoon`
  globals. Needs a day/night cycle — no sensible value to set today.
- **Shore-edge shader using `edgeTexturePath`.** Water variants have a
  dedicated shore-tile atlas (`edgeTexturePath` in the XML, already
  parsed onto `TerrainDef`) that RimWorld uses instead of the
  noise-masked FadeRough edge when the neighbor is a hard terrain.
  We still fall through the FadeRough branch. Wire up in
  `compute_terrain_edge_contributions` when the softened shore reads
  as too mushy.
- **Splash flecks contributing to the depth RT.** `Graphic_FleckSplash`
  splashes are rendered into the same WaterDepth RT in RimWorld, so
  pawns stepping in water briefly deepen the local depth value. Needs
  a fleck system.
- **Subcamera one-frame lag replication.** RimWorld's depth RT is a
  frame old; wgpu lets us run both passes in the same encoder. We
  sequence `[depth → offscreen] → [main]` instead. No reason to
  reproduce Unity's ordering accident.
- **Pixel-match fidelity with RimWorld's actual shaders.** The
  `Map/TerrainWater.shader` and `Map/WaterDepth.shader` are binary
  Unity assets and not in the decompile. Our shaders are
  approximations; the bar is "reads as water". A pixel-diff tuning
  pass is a separate project.
- **`ThingDef` XML inheritance fix.** Same root cause as the
  `TerrainDef` inheritance fix that landed here (`e358048`), but
  larger blast radius. Would need to audit every ThingDef that
  currently fails to load and make sure the renderer handles them
  correctly. Separate commit.

## Tuning handles (hard-coded today, may want to parameterize)

All in `src/water_surface.wgsl`:

- **`SKY_TINT = vec3(0.45, 0.65, 0.85)`** — the daylight sky color the
  reflection texture gets multiplied by. Packed `Other/WaterReflection`
  is grayscale cloud luminosity, not pre-colored, so this tint is
  load-bearing. When a day/night cycle lands, this becomes a uniform
  driven by sun angle instead of a constant.
- **`REFLECT_MIX_SHALLOW = 0.35` / `REFLECT_MIX_DEEP = 0.75`** — the
  blend strength of the sky over the ramp base at the shallowest vs
  deepest water. Shallow water lets the mud-bed ramp read through;
  deep water mirrors more sky.
- **`REFLECT_SCALE = 8.0`** — world units per sky tile. Larger = fewer
  visible repeats (softer, more static); smaller = tighter shimmer.
- **`RIPPLE_SCALE = 3.5`** — world units per ripple tile. Larger =
  broader, slower-looking ripples; smaller = tighter shimmer.
- **`RIPPLE_REFLECT_STRENGTH = 0.035` /
  `RIPPLE_DEPTH_STRENGTH = 0.03`** — how much `Other/Ripples`
  distorts the sky reflection lookup and ramp-depth lookup. Keep these
  subtle; the depth RT should still be the main water shape.

And in `src/water_assets.rs::water_shader_params`:

- **Per-type `depth_const` (0.35 / 0.75 / 0.9)** — drives the write
  into the R16Float depth RT, which then indexes both the ramp and
  the reflection-strength mix. These are approximations of RimWorld's
  opaque depth math; tuned visually. If deep water ever needs to look
  distinctly deeper than chest-deep (or vice versa), this is the
  first knob.

## Adjacent bugs noticed during this work

- **`ThingDef` XML inheritance is not resolved.** Same pattern as the
  `TerrainDef` bug we fixed in `e358048`: any def whose `graphicData`
  or other fields come from an `Abstract="True"` parent via
  `ParentName` is silently dropped or rendered with defaults. Fix is
  analogous — a two-pass resolver before finalizing — but the blast
  radius is much larger (hundreds of ThingDefs, every render path has
  to be re-checked). Scope a separate project; flag at high priority
  when a concrete visual regression pins down which defs are broken.
- **Edge-fan suppression at water↔water boundaries.** Fixed in
  `8cd363e`: when both sides of a cell boundary have
  `water_depth_shader` set, the edge-fan system now skips emission.
  Without this the higher-precedence water's ramp gets overlaid on
  the lower one's perimeter as a muddy band inside the water body.

## Completed followups

- **Ripple distortion.** Landed after the original water branch: the
  surface pass now binds `Other/Ripples`, samples two animated scrolls,
  keys faster movement from `tint.b` / `_UseWaterOffset`, and uses the
  result for subtle reflection and ramp-depth shimmer.

## Known rough edges

- **Fragment recomputes `screen_uv` from `clip_pos`** in
  `water_surface.wgsl` using the stored screen size in the camera
  uniform. Fine on a fixed surface but an aliasing risk if the
  swapchain ever resizes mid-frame. Renderer `resize` does recreate
  the depth target and camera uniform synchronously, so this is
  correct today — flag here in case someone introduces async
  resize handling.
- **Reflection sampler uses `Repeat` addressing**; if the reflection
  texture ever acquires sharp seams at edges, swap to `Mirror`.
- **`_NoiseTex` global** (`Other/Noise`, per `Verse/TexGame.cs:22`)
  is never loaded. We only need `_AlphaAddTex` and the ramps + ripple
  + reflection. Will come up if any future shader needs generic
  noise.

## Reference screenshots

```
cargo run -- fixture fixtures/v2/water.ron \
    --screenshot plans/water-rendering/reference/water.png \
    --no-window

cargo run -- fixture fixtures/v2/water_smoke.ron \
    --screenshot plans/water-rendering/reference/water_smoke.png \
    --no-window
```

Regenerate when touching `water_depth.wgsl`, `water_surface.wgsl`,
`water_assets.rs`, or either fixture. `terrain_mix.ron` also exercises
the water path now — regen its reference in
`plans/terrain-walls-linking/reference/terrain_mix.png` under the same
rules.
