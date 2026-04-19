# Water Rendering — Implementation Plan

Stand up a two-pass water rendering pipeline modeled on RimWorld's
`SectionLayer_Watergen` + `TerrainWater` system, plus the prerequisite
fixes that currently prevent water terrains from even loading.

Deferred from `plans/terrain-walls-linking/followups.md` §"Water terrain
rendering (base pass + depth pass)". Companion references:
`~/rimworld-decompiled/MAP/INDEX.md`,
`~/rimworld-decompiled/Verse/SectionLayer_Watergen.cs`,
`~/rimworld-decompiled/Verse/TerrainDef.cs:352-458`,
`~/rimworld-decompiled/Verse/WaterInfo.cs`,
`~/rimworld-decompiled/Verse/SubcameraDriver.cs`.

---

## 1. Goals

- Render water terrains (`WaterShallow`, `WaterDeep`, `WaterOcean*`,
  `WaterMovingShallow`, `WaterMovingChestDeep`, `Marsh`) as something
  that visually reads as water, not as a muddy ramp gradient.
- Structure the renderer so it has a reusable offscreen color
  render-attachment pass, not only a single swapchain pass. Water is the
  first user; future fog-of-war, splash flecks, glow, and distortion
  passes will reuse the primitive.
- Fix the TerrainDef XML inheritance bug so `WaterDeep`,
  `WaterMovingChestDeep`, `WaterOceanDeep`, and ~26 other abstract-child
  terrain defs load at runtime (currently 29 / 57 load).
- Fix the `RoughAlphaAdd` asset-path mismatch so FadeRough and water
  edges sample the real noise mask instead of the 1×1 gray fallback.
- Ship a static-animated first cut — time-driven ripple distortion is
  fine; sun/moon specular, river flow, and splashes are separate
  follow-ups.

## 2. Non-Goals (Explicitly Deferred)

| Deferred | Trigger for un-deferring |
|---|---|
| River flow `_WaterOffsetTex` animation for `WaterMovingShallow` / `WaterMovingChestDeep` | When a fixture needs visible river flow direction |
| Sun/moon specular (`_WaterCastVectSun` / `_WaterCastVectMoon`) | When a day/night cycle / skylight system exists |
| Shore-edge shader using `edgeTexturePath` (parsed but unused in terrain edge fans, per `followups.md`:70-73) | When a water fixture needs crisp shore tiles rather than noise-masked fades |
| Splash flecks contributing to the depth RT (`Graphic_FleckSplash`) | When a fleck system is built |
| Pixel-match fidelity with RimWorld's actual shaders | Bar is "reads as water"; pixel-diff bar is a separate tuning pass |
| Subcamera one-frame lag replication | wgpu lets us do both passes in-frame; no reason to reproduce Unity's ordering accident |
| `ThingDef` XML inheritance fix | Same root cause as the TerrainDef bug but larger blast radius; separate commit |

## 3. Research Summary (Spec Reference)

### 3.1 Pipeline shape (source-backed)

RimWorld uses **one offscreen color RT + the normal main-camera target**
— not two offscreen buffers composited by a third pass.

- `SectionLayer_Watergen` (a subclass of `SectionLayer_Terrain`,
  `Verse/SectionLayer_Watergen.cs:6-34`) draws water quads with
  `terrain.def.waterDepthMaterial` and routes them to
  `SubcameraDefOf.WaterDepth.LayerId` via
  `Graphics.DrawMesh(...,layerId)`. Only the WaterDepth subcamera's
  culling mask includes that layer, so only that camera renders those
  meshes.
- The WaterDepth subcamera (`Verse/SubcameraDriver.cs:14-40`) has
  `depth=100`, its own screen-sized `RenderTexture` with format
  `RFloat` (fallback chain `RFloat → RG16 → ARGB32` per
  `Verse/SubcameraDef.cs:36-59`), and clears to `(0,0,0,0)`.
- The main camera renders terrain normally via `SectionLayer_Terrain`.
  For water cells `TerrainDef.Shader` returns
  `ShaderDatabase.TerrainWater` (`Verse/TerrainDef.cs:352-378`). The
  resulting material samples the WaterDepth RT as a global texture
  `_WaterOutputTex` in screen space.
- Globals bound once per map tick by `WaterInfo.SetTextures()`
  (`Verse/WaterInfo.cs:41-93`, called from `Verse/Map.cs:1170`):
  `_WaterOutputTex`, `_WaterReflectionTex`, `_MainCameraScreenParams`,
  `_MainCameraVP`, `_WaterOffsetTex`, `_MapSize`.
- `_GameTime` is set by `RimWorld.Planet/GlobalRendererUtility.cs:8-11`
  (`Shader.SetGlobalFloat(ShaderPropertyIDs.GameTime, TicksGame/60)`),
  called from `Verse/Map.cs:1168` every map render.
  `RimWorld.Planet` namespace is misleading — the caller is
  `Verse/Map.cs`. Drives all time-animated shaders, water included.
- Startup globals bound once by `Verse/TexGame.cs:18-28`: `_NoiseTex`
  (`Other/Noise`), `_RippleTex` (`Other/Ripples`),
  `_SpecularNoiseTex` (`Other/SpecularMetal`), plus
  `TexGame.AlphaAddTex` (`Other/RoughAlphaAdd`) which is written
  per-material rather than globally.

### 3.2 Two-pass timing

Unity runs cameras in ascending `depth`. Main camera typically renders
at `depth≈0`; WaterDepth subcamera at `depth=100` renders after. That
means the main camera's surface pass reads **last frame's** WaterDepth
RT, not current frame's — a one-frame lag that RimWorld accepts
silently.

**We do not reproduce this.** In wgpu we sequence two render passes in
the same frame: `[depth → offscreen RT] → [main → swapchain, sampling
offscreen RT]`. Single-frame, no lag.

### 3.3 Material construction

`TerrainDef.PostLoad` (`Verse/TerrainDef.cs:346-458`) does:

1. Base graphic:
   `graphic = GraphicDatabase.Get<Graphic_Terrain>(texturePath, Shader, Vector2.one, DrawColor, 2000 + renderPrecedence)`.
2. If `edgeType` is `FadeRough` or `Water`, sets
   `graphic.MatSingle.SetTexture("_AlphaAddTex", TexGame.AlphaAddTex)`
   (lines 433-437).
3. If `waterDepthShader` is non-null, creates a second material:

```csharp
waterDepthMaterial = MaterialAllocator.Create(
    ShaderDatabase.LoadShader(waterDepthShader));
waterDepthMaterial.renderQueue = 2000 + renderPrecedence;
waterDepthMaterial.SetTexture("_AlphaAddTex", TexGame.AlphaAddTex);
// + apply each entry in waterDepthShaderParameters (e.g. _UseWaterOffset=1)
```
(`Verse/TerrainDef.cs:446-458`).

### 3.4 Concrete water terrain XML

From `Core/Defs/TerrainDefs/Terrain_Water.xml`, read via the first
research pass:

| defName | `texturePath` | `edgeType` | `waterDepthShader` | `renderPrecedence` |
|---|---|---|---|---|
| `WaterShallow` | `Terrain/Surfaces/WaterShallowRamp` | Water | `Map/WaterDepth` | 394 |
| `WaterDeep` | `Terrain/Surfaces/WaterDeepRamp` | Water | `Map/WaterDepth` | 395 |
| `WaterOceanShallow` | `WaterShallowRamp` | Water | `Map/WaterDepth` | 396 |
| `WaterOceanDeep` | inherits | Water | `Map/WaterDepth` | 397 |
| `WaterMovingShallow` | `WaterShallowRamp` | Water | `Map/WaterDepth` (+ `_UseWaterOffset=1`) | 398 |
| `WaterMovingChestDeep` | inherits | Water | `Map/WaterDepth` (+ `_UseWaterOffset=1`) | 399 |
| `Marsh` | `Terrain/Surfaces/Marsh` | FadeRough | — | 325 |

Marsh intentionally uses `FadeRough` — it gets `_AlphaAddTex` but no
depth pass.

### 3.5 Shader source — unknown

`Map/TerrainWater.shader` and `Map/WaterDepth.shader` are binary Unity
asset bundles. **They are not in the decompile.** The C# tells us the
inputs and outputs; the math is opaque. Phase 3 is
approximate-and-tune, not transliteration.

Known inputs:
- **Depth pass** writes a per-pixel single-channel float. Conceptually
  a "water depth" value modulated by `_AlphaAddTex` for rough edges,
  optionally offset by `_UseWaterOffset` × river flow.
- **Surface pass** reads the depth RT (`_WaterOutputTex`) in screen
  space via `_MainCameraScreenParams` and `_MainCameraVP`, reads
  `_MainTex` as a **gradient ramp** keyed by depth (not a tileable
  texture — the ramps are 1D-ish color strips), applies
  `_AlphaAddTex` for edge softness, `_RippleTex` + `_GameTime` for
  animated wave distortion, `_WaterReflectionTex` for sky reflection.

### 3.6 Required texture assets

| Asset (RimWorld path) | Where read from (C#) | Role |
|---|---|---|
| `Other/RoughAlphaAdd` | `Verse/TexGame.cs:20` | `_AlphaAddTex` noise mask (also reused by FadeRough edges) |
| `Other/Ripples` | `Verse/TexGame.cs:21` | `_RippleTex` wave distortion |
| `Other/Noise` | `Verse/TexGame.cs:22` | `_NoiseTex` generic |
| `Other/WaterReflection` | `Verse/WaterInfo.cs:26,50` | `_WaterReflectionTex` sky reflection |
| `Terrain/Surfaces/WaterShallowRamp` | XML | surface color ramp for shallow |
| `Terrain/Surfaces/WaterDeepRamp` | XML | surface color ramp for deep |
| `Terrain/Surfaces/WaterChestDeepRamp` | XML | surface color ramp for chest-deep |

`Marsh` uses `Terrain/Surfaces/Marsh` directly as a tileable base — no
ramp.

## 4. Current State

### 4.1 What we have

- Single swapchain render pass (`src/renderer.rs:807-916`), two
  pipelines (base + edge), pipeline-switching sort by `min_z`.
- Flat `SpriteBatch` / `EdgeSpriteBatch` draw model, no section
  batching.
- Sampled-texture infrastructure with content-hash dedup
  (`src/renderer.rs:35-37, 581-594`).
- Noise texture loaded for edge shader
  (`src/commands/fixture_cmd.rs:50-64`).
- 9-vertex fan edge overlay system (`src/edge_shader.wgsl`,
  `src/commands/linking_sprites.rs:245-394`).
- `TerrainEdgeType::Water` enum exists; currently mapped to a branch in
  `src/edge_shader.wgsl:68-69` that is a literal copy of `FadeRough`.

### 4.2 What we are missing

- **No offscreen color render attachment path.** `Renderer::render`
  opens exactly one `begin_render_pass` against the swapchain.
- **No multi-pass frame orchestration.** Everything is sorted into one
  pass.
- **`TerrainDef`** (`src/defs.rs:114-121`) has no `water_depth_shader`,
  no `water_depth_shader_parameters`.
- **`parse_terrain_def`** (`src/defs.rs:574-602`) does not resolve
  `ParentName` / `Abstract="True"` inheritance. Any def whose
  `texturePath` is inherited is dropped. 29 / 57 terrain defs load
  today. Missing at minimum: `WaterDeep`, `WaterOceanDeep`,
  `WaterMovingChestDeep`, several metal tiles, stone-tile variants.
  `load_head_type_defs` (`src/defs.rs:318-376`) already implements a
  two-pass resolver for the `HeadTypeDef` case — template exists.
- **`RoughAlphaAdd` path cosmetically off.**
  `src/commands/fixture_cmd.rs:30` uses `"Things/Misc/RoughAlphaAdd"`;
  RimWorld uses `"Other/RoughAlphaAdd"` (`Verse/TexGame.cs:20`).
  **Verified during implementation**: the packed resolver matches on
  basename only (`src/assets/variants.rs:16-21`), so both paths
  resolve to the same 256×256 asset and there is no silent gray
  fallback — earlier speculation in
  `plans/terrain-walls-linking/plan.md:534` did not hold up. Phase −1
  is therefore cosmetic alignment, not a bug fix. Still worth doing
  so the path is greppable against the decompile.
- **No time uniform.** Camera uniform (`CameraUniform` in
  `src/renderer.rs`) has no frame-time field.
- `fixtures/v2/terrain_mix.ron` uses `Ice` where a water pocket belongs
  (explicit placeholder, noted in that file's header comment).

## 5. Plan

Commit-granular, ordered by dependency. Each phase is its own commit
and its own PR-shaped unit of review.

### Phase −1 — Align `RoughAlphaAdd` asset path with RimWorld source

**Change.** `src/commands/fixture_cmd.rs:30`:
`"Things/Misc/RoughAlphaAdd"` → `"Other/RoughAlphaAdd"`. Update the
accompanying doc comment.

**Investigation finding (done during implementation).** The packed
resolver matches on **basename** — `src/assets/variants.rs:16-21`
lowercases just the last path segment and looks it up in
`keys_by_name`. Both `Things/Misc/RoughAlphaAdd` and
`Other/RoughAlphaAdd` produce the basename `roughalphaadd`, which
hits `resources.assets::3230` — the real 256×256 mask. There is **no
visual bug** and the 1×1 gray fallback is **not** active today.

This phase is therefore a cosmetic alignment only: match
`Verse/TexGame.cs:20` so the path is greppable against the decompile,
and future-proof against resolver changes that become prefix-aware.

**Verification.**
1. Run `cargo run --release -- fixture fixtures/v2/terrain_mix.ron
   --no-window --screenshot /tmp/tm.png`. No `noise texture ... not
   resolved` warning in logs.
2. Byte-compare the pre/post screenshots — they are identical
   (confirmed during implementation; basename resolver means no
   pixel-level change).
3. `cargo fmt --check && cargo clippy && cargo test` clean.

**Non-goal.** Screenshot regeneration — nothing moves. No renderer
architecture changes.

---

### Phase 0 — TerrainDef XML inheritance resolver

**Change.** Rewrite `parse_doc_terrain_defs` / `load_terrain_defs` in
`src/defs.rs` as a two-pass loader mirroring `load_head_type_defs`
(`src/defs.rs:318-376`):

1. Pass 1: read every `<TerrainDef Name="..." ParentName="..."
   Abstract="...">` into an intermediate node map keyed by `Name`,
   preserving the raw XML children.
2. Pass 2: resolve each non-abstract node by walking its `ParentName`
   chain, merging child XML children over parent children (child
   wins), then run the existing `parse_terrain_def` on the merged
   result.
3. Abstract nodes contribute attributes but do not themselves produce
   `TerrainDef` values.

**Edge cases.**
- Cyclic `ParentName` chains: log + skip; do not infinite-loop.
- Missing parent: log + use child as-is (don't drop — match RimWorld's
  tolerant behaviour).
- `texturePath` overridable in children (e.g. Ocean variants override
  the ramp texPath).

**Verification.**
1. Add a unit test under `src/defs.rs`: loader returns
   `WaterDeep`, `WaterOceanDeep`, `WaterMovingChestDeep` with correct
   `texture_path` and `render_precedence`. Today the test would fail
   with "def not found".
2. `cargo run -- fixture fixtures/v2/terrain_mix.ron --no-window` logs
   a terrain count of 57 (not 29). Drop a debug count log temporarily;
   remove before merge.
3. Run the full test suite — any test that asserted the broken count
   should be updated, not retained.

**Non-goal.** `ThingDef` inheritance. Same bug class, larger blast
radius, separate commit.

---

### Phase 1 — Extend `TerrainDef`, load water support textures

**Change A.** `src/defs.rs:114-121` — add fields:
```rust
pub water_depth_shader: Option<String>,
pub water_depth_shader_parameters: Vec<(String, f32)>,
```
Extend `parse_terrain_def` to read `<waterDepthShader>` (string) and
`<waterDepthShaderParameters>` (list of `<Name>Value</Name>` children;
parse values as `f32`).

**Change B.** Add a new module (`src/water_assets.rs` or similar)
that, at renderer init, resolves and caches:
- `Other/Ripples` → `_RippleTex`
- `Other/WaterReflection` → `_WaterReflectionTex`
- `Terrain/Surfaces/WaterShallowRamp`
- `Terrain/Surfaces/WaterDeepRamp`
- `Terrain/Surfaces/WaterChestDeepRamp`

Each with a `fallback_*` small-texture counterpart (solid color or 1×1
chequer) so the renderer starts up cleanly when an asset can't be
resolved. Log at `warn` on fallback, not error.

**Verification.**
1. Unit test: parse a synthetic `<TerrainDef>` XML with
   `waterDepthShader` + `waterDepthShaderParameters`; struct fields
   come out correct.
2. Start the fixture runner and assert the water-assets module
   resolves all six paths without hitting a fallback, or warns for
   each missing one. No `error!` level log.

**Non-goal.** Do not wire the assets into any pipeline yet. This
phase is parse + load only.

---

### Phase 2 — Offscreen render attachment + two-pass orchestration

**Change A.** `Renderer` gains:
- `water_depth_target: wgpu::Texture` with format `R16Float`, viewport
  size, `RENDER_ATTACHMENT | TEXTURE_BINDING` usage, recreated on
  `resize`.
- `water_depth_view: wgpu::TextureView`.
- `water_depth_bind_group` (surface pass binds this as a sampled
  texture in a new `@group(N)` slot).
- `water_depth_pipeline: wgpu::RenderPipeline` — vertex format TBD in
  Phase 3; in this phase, stub with a pipeline that writes a constant
  value so we can prove plumbing.

**Format choice.** Start at `R16Float`. `R32Float` matches RimWorld's
`RFloat` but is overkill; `R8Unorm` risks visible banding in shore
gradients. Downgrade to `R8Unorm` only if a wgpu target platform
doesn't support `R16Float` sampling (rare). Do not start with
`R32Float` — it costs 4× the memory for imperceptible benefit at our
scale.

**Change B.** `Renderer::render` restructures into two sequential
render passes in a single command encoder:

```
encoder.begin_render_pass(water_depth_target, Clear=(0,0,0,0))
    draw water-depth pipeline for water cells
end_pass

encoder.begin_render_pass(swapchain, Clear=clear_color)
    draw base pipeline for non-water sprites
    draw edge pipeline for edge fans
    draw water-surface pipeline for water cells (samples water_depth_view)
end_pass
```

Sort/interleave logic within the swapchain pass preserves the
current `min_z` merge. Water-surface draws are inserted at the water
terrain `min_z` — same depth as `-1.0` for terrain.

**Change C.** Add `frame_time_seconds: f32` to `CameraUniform`,
updated per-frame by the runtime (start at `Instant::now()` subtraction
from a fixed epoch; v2 runtime can swap to in-game ticks later).

**Verification.**
1. Stub water-depth pipeline writes `1.0` everywhere; stub surface
   shader samples it and tints water cells bright red. Fixture run
   shows red where water is — proves the RT is wired up end-to-end.
2. `cargo test` and `cargo clippy` clean.
3. Resize the window; no wgpu validation errors about stale attachment
   sizes.

**Non-goal.** No ramp sampling, no ripple, no `_AlphaAddTex`. Pure
plumbing with a smoke-test shader.

---

### Phase 3 — Real water shaders (`water_depth.wgsl`, `water_surface.wgsl`)

**Open-ended — this is the approximation phase.**

**Change A.** `src/water_depth.wgsl` — inputs:
- Vertex: same instanced quad as base terrain.
- Fragment uniforms: `_AlphaAddTex` (noise), `_UseWaterOffset: f32`,
  `frame_time`.
- Output: single-channel float in `[0, 1]` representing a per-pixel
  water-depth-ish value. Approximation: base constant per water-type
  (0.3 for shallow, 0.7 for deep, 0.9 for chest-deep) modulated by
  `_AlphaAddTex` at a cell-local UV, offset by `_UseWaterOffset *
  sin(frame_time + uv)` for moving variants. Values will be tuned; the
  shape is what matters.

**Change B.** `src/water_surface.wgsl` — inputs:
- Vertex: same instanced quad.
- Uniforms: `_MainTex` (the ramp), `_AlphaAddTex`, `_RippleTex`,
  `_WaterOutputTex` (our offscreen RT), `_WaterReflectionTex`,
  camera + `frame_time`.
- Fragment:
  1. Compute screen-UV from fragment world position + camera VP.
  2. Sample `_WaterOutputTex` at screen-UV → `d ∈ [0,1]`.
  3. Sample `_MainTex` (ramp) using `d` as the X coordinate → base
     color.
  4. Sample `_RippleTex` with `uv + frame_time * ripple_scroll` → a
     displacement value; offset base color sample UV by it.
  5. Sample `_WaterReflectionTex` with world-space UV; blend in at
     low opacity.
  6. Apply `_AlphaAddTex` to soften the near-shore alpha
     (`d` close to 0).

**Tuning notes.**
- Start with the shallowest water only. Get `WaterShallow` looking
  right before touching `Deep` / `ChestDeep` / `Moving`.
- Animation speed / ripple scroll is tuned visually — expect several
  iterations. Don't wire sun/moon lighting here.
- If the depth-sampling-in-screen-space feels wrong in wgpu (e.g. flipY
  issues), cross-check against RimWorld's expected Y-orientation —
  `GL.GetGPUProjectionMatrix(..., renderIntoTexture:false)` at
  `Verse/WaterInfo.cs:57` forces the non-RT-flipped matrix, which
  matters.

**Verification.**
1. `fixtures/v2/terrain_mix.ron`: swap `Ice` pocket back to
   `WaterShallow`. Renders as water — not muddy brown gradient, not
   red, and shore cells blend into grass/sand with noise-masked edge
   from the existing edge fan system.
2. New fixture `fixtures/v2/water.ron` exercising `WaterShallow`,
   `WaterDeep`, and a small `WaterMovingShallow` patch (no flow map
   yet — just confirms the shader parameter path applies). Add
   screenshot under `plans/water-rendering/reference/water.png`.
3. No regression in existing reference PNGs (grass-meets-sand edges,
   walls, pawns).

**Risk.** This phase is unbounded in effort. Budget: one concentrated
session to get shallow water "reads as water"; followups for tuning,
deep/chestdeep, and moving-water animation. If by end of session the
output isn't plausibly water, retreat to a simpler solid-color
animated noise surface and fall back to the baked-in gradient ramp
without the two-pass machinery — but keep the two-pass machinery
shipped, because it's independently valuable.

---

### Phase 4 — Fixture, reference, followups refresh

**Change.**
- Swap `fixtures/v2/terrain_mix.ron` pond from `Ice` back to
  `WaterShallow`. Update its header comment accordingly.
- Add `WaterDeep` cells alongside — now that inheritance works, they
  actually load.
- Add `fixtures/v2/water.ron` (small, focused): shore cells, shallow
  pool, deep center, one moving-water strip.
- Regenerate `plans/terrain-walls-linking/reference/terrain_mix.png`.
- Create `plans/water-rendering/reference/water.png`.
- Update `plans/terrain-walls-linking/followups.md`:
  - Remove the water entry from "Deferred by original plan".
  - Remove the inheritance bug from "Adjacent bugs noticed" (note in
    commit message which phase landed it).
  - Note the RoughAlphaAdd path fix (or just let git history record
    it).
- Add `plans/water-rendering/followups.md` capturing what this feature
  deferred (river flow, sun/moon specular, shore-tile edges, splash
  flecks, pixel-match fidelity, ThingDef inheritance).

**Verification.**
- `cargo run -- fixture fixtures/v2/water.ron --no-window
  --screenshot plans/water-rendering/reference/water.png`. Commit both.
- Confirm `cargo test` still passes.

---

## 6. Risks

### 6.1 Unknown shader math (highest risk)

`TerrainWater` and `WaterDepth` are binary Unity shader assets not in
the decompile. Phase 3 is approximate-and-tune. Mitigation: ship an
explicit "this is an approximation, bar is 'reads as water'" commit
message, and accept that pixel-match is a later project.

### 6.2 Asset path resolution

The packed index may not expose `Other/Ripples`, `Other/WaterReflection`
etc. under the RimWorld-native paths. The resolver may need a small
mapping table or baked-in fallbacks. Log-at-warn + fallback-textures in
Phase 1 keeps this from blocking pipeline work.

### 6.3 Screen-space sampling of the depth RT

wgpu's NDC Y is top-down, Unity's is bottom-up, OpenGL's is
bottom-up. `Verse/WaterInfo.cs:57` uses
`GL.GetGPUProjectionMatrix(..., renderIntoTexture:false)` to normalize
the VP matrix for screen-UV sampling. Our WGSL needs to handle the Y
flip explicitly in the screen-UV computation. Smoke test in Phase 2
catches this early.

### 6.4 Inheritance resolver subtleties

Some terrain defs may have odd patterns (inherit from ThingDef
parents, have mod extensions, etc.). The `HeadTypeDef` resolver is
likely missing some edge cases that only `TerrainDef` exposes. Unit
test per-def after Phase 0 to catch silent drops.

### 6.5 Scope creep into `ThingDef` inheritance

Tempting to fix `ThingDef` inheritance while fixing `TerrainDef` —
don't. Larger blast radius, more tests to write, more defs to
validate. Separate commit, after water ships.

## 7. Testing Strategy

- **Unit tests**: Phase 0 (def count 57 + spot-check water defs),
  Phase 1 (XML field parsing), Phase 2 (smoke-test fragment output).
- **Fixture screenshots**: every phase that touches rendered output
  regenerates the three existing reference PNGs plus
  `water.png` from Phase 4. Human visual inspection is the regression
  signal — pixel-diff harness is deferred per
  `plans/terrain-walls-linking/followups.md:85-89`.
- **`cargo clippy` + `cargo test` clean at every phase boundary.** Per
  `AGENTS.md` lint policy.
- **No `_var` warning-silencers.** Per `CLAUDE.md`: remove dead
  parameters, don't `_`-prefix them.

## 8. Open Questions

- Do our packed asset paths match RimWorld's `Other/...` prefix
  literally, or does the resolver need a mapping layer? (Phase −1 +
  Phase 1 will answer.)
- Ramp textures — gradient strips or proper 2D textures? The XML calls
  them `Ramp` and they're tagged at RimWorld's standard res; inspect
  the extracted bytes in Phase 1 before writing the ramp sampler.
- Does `_MapSize` matter for any approximation we'd build? RimWorld
  uses it alongside `_WaterOffsetTex` for cell-indexed flow lookup;
  without flow, we don't need it. Defer.
- Do we want `frame_time` in `CameraUniform` or a separate global UBO?
  `CameraUniform` is simpler now; a `GlobalsUniform` becomes warranted
  once we have more than one time-driven shader. Start simple.
