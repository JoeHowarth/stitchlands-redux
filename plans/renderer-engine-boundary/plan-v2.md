# Renderer / Engine Boundary — Implementation Plan (v2)

Companion to `plan.md`. Same overall direction (mechanical split →
neutral types → policy-leak fixes → shared scene assembly → explicit
pass queue), with concrete citations, a four-way `Renderer` split,
and an opinionated stance on `Vec<Box<dyn Pass>>` vs. a concrete
`RenderPassStep` enum.

## Revisions

- **r1**: Initial draft.
- **r2**: Phase 1 construction order corrected (`TextureRegistry`
  before `PipelineSet`); Phase 2 reworked so scene types do not
  depend on renderer-private types (introduces `SceneTexture` enum,
  moves `TextureId` → `scene::TextureHandle`); Phase 3 split into
  `StaticSceneBuilder` + `LiveFrameBuilder` to preserve the existing
  static/dynamic cadence; animation state moved from `RenderState`
  fields to a `LiveFrameContext` parameter; scene builders no longer
  borrow `TextureRegistry` — renderer ingests scene records and
  performs uploads.
- **r3**: Material-role naming folded into Phase 2 (was previously
  deferred to a hypothetical Phase 5). `scene::MaterialKind` enum
  added alongside `scene::Layer`; `Layer` expanded to make the world
  batches (`Terrain`, `StaticThings`, `Dynamic`) first-class
  variants; `LayerOrdering` enum specifies per-layer within-layer
  sort rule (precedence is layer-specific, not global). Phase 1
  picks up the named parity fixes (LightOverlay blend mode,
  SunShadowFade pipeline) as soon as `PipelineSet` exists. Phase 4's
  `RenderPassStep` dispatches on `MaterialKind` for pipeline routing
  (independent axis from `Layer`) and applies per-layer
  `LayerOrdering` when draining batches. Reference roadmap:
  `~/rimworld-shader-extract/INDEX.md`.

Reshape the renderer + scene assembly layers so the codebase reads as
"engine + app" instead of "rendering code smashed together," driven by
the two real entry points we have today (static fixture command, live
v2 runtime) rather than by an abstract architecture goal.

This plan is medium-scope: rearrange and split, with one targeted
unification step. It explicitly does **not** introduce a frame graph,
ECS, plugin system, generic scene IR, or a separate engine crate.

---

## 1. Goals

- Replace the 2,408-line `src/renderer.rs` god struct with a renderer
  module split by responsibility (GPU context vs. static pipelines vs.
  texture cache vs. frame execution), so adding a feature like water
  doesn't require editing one 2k-line file.
- Move the renderer's neutral CPU-side input types (`SpriteInput`,
  `ColoredMeshInput`, `TexturedMeshInput`, etc.) and the `OverlayPass`
  enum out of `src/renderer.rs` into a `src/scene/` module — they are
  app-shaped data, not GPU machinery.
- Unify the two scene-assembly paths (`commands/fixture_cmd.rs` for
  the static entry point, `runtime/v2/render_bridge.rs` for the live
  tick loop) onto a **shared `build_static_scene`** plus a separate
  **`build_live_frame`** invoked per tick. The static builder runs
  once per fixture/runtime startup; the live builder runs per tick
  and produces only dynamic/animated records. **Load-bearing step**:
  without it, the two paths drift as the live runtime grows
  animation/effects features.
- Turn the implicit ordered draw sequence in `Renderer::render`
  (`src/renderer.rs:1449`) into an explicit `RenderPassStep` enum with
  named offscreen targets, so water-style multi-pass features stop
  needing surgery on `Renderer::new` and `Renderer::render` together.
- Replace `OverlayPass` (a renderer-internal enum imported by every
  overlay builder under `src/commands/`) with a scene-level `Layer`
  enum living next to the neutral scene types, plus a
  `MaterialKind` enum that names the RimWorld-style render role of
  each scene record (Cutout, Terrain, TerrainEdge, TerrainWater,
  WaterDepth, LightOverlay, EdgeShadow, SunShadow, FogOfWar, Snow,
  Transparent, SolidColor). Replace the existing `is_water` /
  `is_terrain` booleans in renderer routing with `MaterialKind`
  matches.

## 2. Non-Goals (Explicitly Deferred)

| Deferred | Trigger for un-deferring |
|---|---|
| Separate `stitchlands-engine` crate / Cargo workspace | When a second binary or external consumer of the engine appears |
| Generic `RenderScene` IR with `MaterialId` / `DrawSprite` indirection over the existing CPU input types | When a second renderer backend (debug wireframe, alternate platform) is actually being built |
| `Pass` trait + `Vec<Box<dyn Pass>>` extensibility | When third-party / mod-authored passes are a real requirement (not anticipated) |
| Dirty tracking / incremental scene mutation in the live runtime | When per-frame rebuild cost shows up in a profile at realistic map sizes |
| ECS / archetype storage for world entities | Out of scope; orthogonal to the engine boundary |
| Frame graph with automatic resource aliasing / barrier insertion | Justified at hundreds of pipelines, not 8 |
| `commands/fixture_cmd.rs` cleanup beyond the scene-assembly extraction | Tracked separately; this plan only pulls scene-building out, not full command-layer restructuring |
| `ThingDef` / `TerrainDef` data flow changes | Out of scope; defs already live cleanly outside the renderer |
| `MaterialKind` subkinds (`CutoutPlant`, `CutoutSkin`, `CutoutHair`, `TransparentPostLight`, `SunShadowFade`, `TerrainFadeRough`) | When a fixture or parity bug requires the distinction. The 12-variant initial set is deliberately coarse |
| Per-shader-file ports of all 193 RimWorld shaders | Goal is ~12 material families (initial working set, not stable taxonomy); motes/gravship/ritual/special-effects deferred indefinitely. See `~/rimworld-shader-extract/INDEX.md` |
| Global `renderPrecedence` sort across all layers | Each `Layer` has its own `LayerOrdering` rule; precedence is the rule for `Layer::Terrain` only |
| `MaterialId` runtime indirection / shader registry | `MaterialKind` is a plain enum; the renderer matches on it. No registry, no IDs |

## 3. Current State

### 3.1 What we have (the good seams)

- **Neutral CPU input types already exist.** `SpriteInput`
  (`src/renderer.rs:2204`), `ColoredMeshInput`
  (`src/renderer.rs:2096`), `TexturedMeshInput`
  (`src/renderer.rs:2105`), `EdgeSpriteInput` (`src/renderer.rs:2231`),
  `OverlayPass` (`src/renderer.rs:2081`), `OverlayBlendMode`
  (`src/renderer.rs:2089`), `SpriteParams` (`src/renderer.rs:2384`)
  contain no `wgpu` types in their fields. Moving them is a relocation,
  not a redesign.
- **`AssetResolver`** (`src/assets/resolver.rs:13`) is already a
  standalone injectable object, owned by `AppContext` and passed into
  scene builders explicitly.
- **`WorldState` / `RenderState`** (`src/world/state.rs:36`) are
  plain-data simulation snapshots. Overlay builders consume them
  read-only.
- **Per-overlay scene builder pattern.** Snow / fog / shadow / lighting
  / glow each follow the same shape: `fn(&WorldState, &mut
  AssetResolver, &Defs) -> Result<Vec<TexturedMeshInput>>` (e.g.
  `src/commands/snow_overlay.rs:40`,
  `src/commands/fog_overlay.rs:20`,
  `src/commands/shadow_overlay.rs:28`,
  `src/commands/lighting_overlay.rs:41`). This is the implicit contract
  the new `SceneBuilder` formalizes.

### 3.2 What we are missing

- **`src/renderer.rs` is one struct doing four jobs.** `Renderer`
  (`src/renderer.rs:26-75`) holds the device/queue/surface, all 8
  pipelines + bind group layouts, the texture cache, *and* the frame
  state. `Renderer::new` (`src/renderer.rs:456-927`) constructs all
  pipelines inline. `Renderer::render` (`src/renderer.rs:1449`) is a
  ~100-line imperative ordered-draw function with hardcoded pass slots.
- **`OverlayPass` leaks renderer internals into commands.** Every
  overlay builder imports it: `src/commands/fog_overlay.rs:5`,
  `src/commands/snow_overlay.rs:4`,
  `src/commands/lighting_overlay.rs:7`,
  `src/commands/shadow_overlay.rs:9`,
  `src/commands/solid_overlay_mesh.rs:3`. Same for the input types in
  `src/commands/mod.rs:44-46`,
  `src/commands/linking_sprites.rs:14`,
  `src/commands/overlays.rs:7`.
- **Two divergent scene-assembly paths.** `commands/fixture_cmd.rs`
  (708 LOC) builds the static scene; `runtime/v2/render_bridge.rs`
  (`src/runtime/v2/render_bridge.rs:6,13,17,28,…`) independently
  produces `SpriteInstance`s for the live tick loop and imports
  `crate::renderer::{FULL_UV_RECT, SpriteInstance, SpriteParams,
  TextureId}` directly. As animation / effects grow on the runtime
  side, these two paths will produce different views of the same
  world.
- **No named offscreen targets.** The water depth pass
  (`src/renderer.rs:1473-1506`) is the only multi-pass feature and its
  RT, view, format constant (`WATER_DEPTH_FORMAT` at
  `src/renderer.rs:24`), bind group, and pass invocation are all
  hand-wired directly on `Renderer`. A second offscreen feature would
  duplicate that wiring rather than reuse it.
- **`is_water` / `is_terrain` style booleans** scattered through the
  draw_world_batches call site (`src/renderer.rs:1524-1550`) instead
  of a `Layer` / `MaterialKind` tag carried with the input.

## 4. Plan

Commit-granular, ordered by dependency. Each phase is its own commit
and PR-shaped review unit. Per `AGENTS.md` §"Work Completion Policy",
every phase ends with `cargo fmt && cargo clippy && cargo test &&
cargo run -- render-fixtures` clean and zero diff in
`fixtures/renders/`.

### Phase 0 — Mechanical split of `src/renderer.rs` into `src/renderer/`

Pure file reorganization. **No behavior changes, no type renames, no
API surface changes for external callers.**

**Change.** Convert `src/renderer.rs` to `src/renderer/mod.rs` and
extract submodules along existing internal boundaries:

| Submodule | Owns |
|---|---|
| `renderer/types.rs` | `SpriteInput`, `SpriteInstance`, `EdgeSpriteInput`, `EdgeFanInstance`, `EdgeFan`, `EdgeType`, `EdgeVertex`, `ColoredMeshInput`, `ColoredVertex`, `TexturedMeshInput`, `TexturedVertex`, `OverlayPass`, `OverlayBlendMode`, `SpriteParams`, `RendererOptions`, `SunShadowParams`, `SunShadowUniform`, `Vertex`, `InstanceData`, `TextureId` |
| `renderer/camera.rs` | `Camera` (`src/renderer.rs:254`), camera uniform, input-handling helpers |
| `renderer/textures.rs` | `TextureKey`, texture HashMap, `register_texture`, hash-dedup logic (`src/renderer.rs:267, 1117`) |
| `renderer/pipelines.rs` | All 8 `wgpu::RenderPipeline` constructors currently inline in `Renderer::new` (`src/renderer.rs:456-927`); shader module loads; bind group layouts |
| `renderer/batches.rs` | `draw_world_batches`, `draw_overlay_pass`, `draw_textured_overlay_pass` |
| `renderer/screenshot.rs` | Readback path (whatever currently produces fixture-render PNGs) |
| `renderer/mod.rs` | `Renderer` struct + `new` + `render` + the public-facing setters |

`pub use` everything from `renderer/types.rs` at `renderer/mod.rs` so
external callers (`commands/`, `runtime/`, `viewer.rs`) see no path
change. **Do not** change any `use crate::renderer::Foo` import sites
in this phase.

**Verification.**
1. `git diff --stat` shows only file moves + `mod.rs`/`pub use` glue;
   no semantic changes.
2. `cargo fmt && cargo clippy && cargo test` clean.
3. `cargo run -- render-fixtures` produces zero diff under
   `fixtures/renders/`.

**Non-goal.** Don't touch `Renderer::new`'s body content, don't
re-shape the struct, don't move types out of the renderer module.
Pure mechanical move.

---

### Phase 1 — Split `Renderer` into four owned components

Now that the file is broken up, split the **type** along the same
lines. `Renderer` becomes a thin coordinator that owns four narrower
structs.

**Change.** Introduce four structs in their respective submodules:

```rust
// renderer/gpu_context.rs
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
    pub surface_config: wgpu::SurfaceConfiguration,
}

// renderer/pipelines.rs
pub struct PipelineSet {
    pub sprite: wgpu::RenderPipeline,
    pub edge: wgpu::RenderPipeline,
    pub colored_overlay: wgpu::RenderPipeline,
    pub textured_overlay: wgpu::RenderPipeline,
    pub sun_shadow: wgpu::RenderPipeline,
    pub water_depth: wgpu::RenderPipeline,
    pub water_surface: wgpu::RenderPipeline,
    // + bind group layouts owned here
}

// renderer/textures.rs
pub struct TextureRegistry {
    map: HashMap<TextureId, wgpu::BindGroup>,
    keys: HashMap<TextureKey, TextureId>,
    next_id: u32,
    layout: wgpu::BindGroupLayout, // shared sprite texture BGL
}

// renderer/frame.rs (renamed from mod.rs's render path)
pub struct FrameRenderer {
    // per-frame batches (moved off Renderer)
    static_sprites: Vec<…>,
    dynamic_sprites: Vec<…>,
    static_overlays: Vec<ColoredMeshInput>,
    static_textured_overlays: Vec<TexturedMeshInput>,
    // offscreen targets (water depth today; one entry, not yet a map)
    water_depth_target: wgpu::Texture,
    water_depth_view: wgpu::TextureView,
}
```

`Renderer` becomes:

```rust
pub struct Renderer {
    gpu: GpuContext,
    pipelines: PipelineSet,
    textures: TextureRegistry,
    frame: FrameRenderer,
    camera: Camera,
}
```

`Renderer::new` reduces to `GpuContext::new(...)` →
`TextureRegistry::new(&gpu)` (creates the shared sprite texture BGL)
→ `PipelineSet::build(&gpu, &textures)` (consumes the BGL) →
`FrameRenderer::new(&gpu)`. `Renderer::render` borrows the four
pieces and threads them into the existing draw sequence — same draw
calls, same order, just routed through narrower owners.

**Construction order matters.** The shared sprite texture
`BindGroupLayout` is owned by `TextureRegistry` (it must be, since
every uploaded texture uses it to create its bind group), and
`PipelineSet` consumes it when constructing pipelines. Inverting
this order means duplicating the BGL or coupling the two structs
through a back-reference; neither is acceptable. Document this
ordering in `renderer/mod.rs::Renderer::new`.

**Why this split, not a single `ResourceRegistry`.** Textures have
runtime dedup tied to scene assets and grow per-fixture; pipelines are
static backend capabilities created once at startup. Different
lifetimes, different change reasons, different test seams. Lumping
them produces the next 2k-line struct with a nicer name.

**Verification.**
1. Public API of `Renderer` (called from `viewer.rs`,
   `commands/fixture_cmd.rs`) is unchanged. Setter methods like
   `register_texture`, `set_static_overlays`,
   `set_static_textured_overlays`, `set_dynamic_sprites` delegate to
   the appropriate inner component.
2. `cargo test && cargo clippy` clean.
3. `cargo run -- render-fixtures` — zero diff.

**Non-goal.** Don't change the `OverlayPass` API yet; don't extract
scene types yet; don't introduce `RenderPassStep`. Internal split
only.

**Parity fixes unblocked by `PipelineSet`.** Once Phase 1 is in,
two named RimWorld parity gaps become cheap and should land as
their own follow-up commits **before Phase 2** (or in parallel,
since they touch only `PipelineSet` + a new shader file each):

- **`LightOverlay` blend mode.** Today's lighting overlay routes
  through `colored_overlay.wgsl` with alpha blending; RimWorld's
  `lighting/lightoverlay.shader` uses `Blend DstColor SrcColor` —
  multiplicative tint where channel values >1 brighten and <1
  darken. Add a dedicated `light_overlay` pipeline in `PipelineSet`
  with the correct blend state; route lighting overlays through it.
- **`SunShadowFade` pipeline.** RimWorld has separate
  `lighting/sunshadow.shader` and `lighting/sunshadowfade.shader`
  pipelines; we currently have only the former. Add the fade
  variant with its distinct blend state. Cross-reference
  `~/rimworld-decompiled/Verse/SectionLayer_SunShadows.cs` and
  `~/rimworld-shader-extract/assetripper-1.3.5-decompile-export/`
  for the authoritative blend states.

Each fix changes pixels intentionally — re-baseline affected fixture
renders in the same commit and call out the visual change in the
commit message. Track these parity items separately from the
boundary refactor; do not bundle.

---

### Phase 2 — Extract neutral scene types into `src/scene/`

**Change.** Create `src/scene/` and move every type that participates
in the scene → renderer contract. The split rule is **dependency
direction**: `scene/` must compile without depending on any
`renderer/` private type. Vertex byte-layout descriptors (the
`bytemuck::Pod` impls + `wgpu::VertexBufferLayout::desc()` methods)
stay renderer-side; the data-shape structs themselves move.

| Stays in `renderer/` | Moves to `scene/` |
|---|---|
| `Vertex`, `InstanceData`, `EdgeFanInstance` (per-instance GPU buffer layout, currently in `src/renderer.rs`) | `SpriteInput`, `SpriteInstance` (renamed → `SpriteRecord`), `EdgeSpriteInput`, `EdgeType`, `EdgeFan` topology helpers if data-only, `ColoredMeshInput`, `ColoredVertex`, `TexturedMeshInput`, `TexturedVertex`, `EdgeVertex`, `OverlayBlendMode`, `SpriteParams`, `SunShadowParams` |
| Vertex `desc()` impls and bytemuck wiring (in a renderer-side adapter module that re-exports the structs from `scene/`) | `TextureId` → renamed `scene::TextureHandle` |
| `RendererOptions` | — |

The vertex structs (`ColoredVertex`, `TexturedVertex`, `EdgeVertex`)
move because their *data* is scene-shaped (positions, colors, UVs);
the renderer keeps the buffer-layout descriptor as a free function
(`fn textured_vertex_layout() -> wgpu::VertexBufferLayout`) that
references the scene type. This avoids the contradiction in r1 where
`ColoredMeshInput` (scene) referenced `ColoredVertex` (renderer).

**Replace `TextureId` with `scene::SceneTexture`.** The bigger
boundary fix: scene records do not carry renderer-private upload
state. Define:

```rust
// src/scene/texture.rs
#[derive(Clone)]
pub enum SceneTexture {
    /// Asset-resolver path; renderer resolves + uploads at ingest.
    Asset(Arc<str>),
    /// Already-decoded image; renderer uploads at ingest.
    Image(Arc<image::RgbaImage>),
    /// Pre-registered (e.g. _AlphaAddTex loaded at startup).
    Handle(TextureHandle),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TextureHandle(pub u32);
```

`SpriteRecord`, `ColoredMeshInput`, `TexturedMeshInput` carry
`SceneTexture` instead of `TextureId`. The renderer ingests scene
records at frame submission: for `Asset`/`Image` variants it resolves
+ content-hash + uploads via `TextureRegistry`, mints a
`TextureHandle`, and patches it into a renderer-side draw record.
Live-path per-frame cost is one HashMap lookup per record (cached by
content hash for `Image`, by path for `Asset`).

This eliminates the r1 contradiction where `SceneBuilder` borrowed
`&mut TextureRegistry` to mint handles inline. Scene assembly is now
GPU-free.

**Replace `OverlayPass` with a fuller `scene::Layer` enum** that
makes the world batches first-class (today they're implicit
between overlay slots). Add a sibling `LayerOrdering` enum so the
within-layer sort rule is layer-specific and explicit:

```rust
// src/scene/layer.rs
pub enum Layer {
    BeforeWorld,
    Terrain,            // was implicit; now explicit
    AfterTerrain,
    StaticThings,       // was implicit
    AfterStatic,
    Dynamic,            // was implicit
    AfterDynamic,
}

pub enum LayerOrdering {
    /// Sort by `TerrainDef::render_precedence`. Used by Layer::Terrain.
    ByTerrainPrecedence,
    /// Sort by (altitude_layer, position.z, render_precedence).
    /// Used by Layer::StaticThings, Layer::Dynamic.
    ByAltitudeThenZ,
    /// Insertion order from the scene record list. Used by overlay slots.
    InsertionOrder,
    /// Caller supplies an explicit z; sort by that. Used by hover/selection markers.
    Explicit,
}

impl Layer {
    pub const fn ordering(self) -> LayerOrdering { /* … */ }
}
```

`FrameRenderer` maps `scene::Layer` → its pass slot when draining
batches and applies `Layer::ordering()` for within-layer sort.
Critical: precedence is **not** a global sort key. Things use
altitude+z, terrain uses precedence, overlays use insertion order.
Centralizing this in `LayerOrdering` keeps the rules visible without
forcing them onto a shared shape.

Commands stop importing `crate::renderer::OverlayPass` and import
`crate::scene::{Layer, MaterialKind}` instead.

**Add `scene::MaterialKind`** to name the RimWorld-style render
role of each scene record. `Layer` and `MaterialKind` are
**orthogonal axes**: `Layer` says *when* a record is drawn,
`MaterialKind` says *which pipeline + blend state*. The renderer
dispatches on `MaterialKind` independently from layer sequencing.

```rust
// src/scene/material.rs
pub enum MaterialKind {
    Cutout,           // map/cutout.shader (covers cutoutcomplex/plant/skin/hair initially)
    Terrain,          // map/terrainhard.shader, map/terrainfade.shader
    TerrainEdge,      // map/terrainedge.shader
    TerrainWater,     // map/terrainwater.shader
    WaterDepth,       // map/waterdepth.shader (offscreen RT pass)
    LightOverlay,     // lighting/lightoverlay.shader (DstColor SrcColor blend)
    EdgeShadow,       // lighting/edgeshadow.shader
    SunShadow,        // lighting/sunshadow.shader
    FogOfWar,         // misc/fogofwar.shader
    Snow,             // misc/snow.shader
    Transparent,      // map/transparent.shader (alpha blend, queue 3000)
    SolidColor,       // map/solidcolor.shader / map/vertexcolor.shader
}
```

Twelve variants matching what current code touches. Subkinds —
`CutoutPlant`, `CutoutSkin`, `CutoutHair`, `TransparentPostLight`,
`SunShadowFade`, `TerrainFadeRough`, etc. — are **explicitly
deferred** until a fixture or bug fix requires the distinction. This
is an initial working set; the taxonomy will evolve as individual
shaders are inspected against
`~/rimworld-shader-extract/assetripper-1.3.5-decompile-export/`.

`SunShadowFade` may land earlier as a Phase 1 parity-fix
follow-up (see Phase 1 §"Parity fixes unblocked by `PipelineSet`").
When it does, add the variant.

Add a `kind: MaterialKind` field to every scene record type
(`SpriteRecord`, `ColoredMeshInput`, `TexturedMeshInput`,
`EdgeSpriteInput`). Migrate every producer in `src/commands/` and
`src/runtime/` to declare its kind. The mapping is mechanical —
each existing builder knows what kind of thing it produces:

| Producer | Current MaterialKind |
|---|---|
| `commands/fog_overlay.rs` | `FogOfWar` |
| `commands/snow_overlay.rs` | `Snow` |
| `commands/lighting_overlay.rs` | `LightOverlay` |
| `commands/shadow_overlay.rs` (sun shadows) | `SunShadow` |
| `commands/shadow_overlay.rs` (edge shadows) | `EdgeShadow` |
| `commands/linking_sprites.rs` (terrain edges) | `TerrainEdge` |
| terrain sprite builder | `Terrain` |
| thing/wall/static-sprite builder | `Cutout` |
| pawn composer (`pawn/`) | `Cutout` |
| water terrain (Phase 3 of `plans/water-rendering/`) | `TerrainWater` + `WaterDepth` |
| hover/selection markers | `SolidColor` |

Drop the `is_water` / `is_terrain` booleans currently in
`renderer.rs:1524-1550` — `MaterialKind` matches replace them.

Update import sites:
- `src/commands/fog_overlay.rs:5`
- `src/commands/snow_overlay.rs:4`
- `src/commands/lighting_overlay.rs:7`
- `src/commands/shadow_overlay.rs:9`
- `src/commands/solid_overlay_mesh.rs:3`
- `src/commands/mod.rs:44-46`
- `src/commands/linking_sprites.rs:14`
- `src/commands/overlays.rs:7`
- `src/runtime/v2/render_bridge.rs:6`

**Verification.**
1. `rg "crate::renderer::" src/scene/` returns zero hits. `scene/`
   compiles with `renderer/` removed from `Cargo.toml`'s module tree
   (verify by temporarily commenting out `pub mod renderer;` in
   `lib.rs` and checking `scene/` still type-checks).
2. `rg "crate::renderer::(SpriteInput|SpriteRecord|ColoredMeshInput|TexturedMeshInput|OverlayPass|OverlayBlendMode|SpriteParams|EdgeSpriteInput|TextureId)" src/`
   returns zero hits outside `renderer/` re-exports.
3. `rg "is_water|is_terrain" src/renderer/` returns zero hits — the
   boolean routing has been replaced by `MaterialKind` matches.
4. Every scene record producer in `src/commands/` and
   `src/runtime/` declares an explicit `MaterialKind`. Spot-check
   each entry from the producer table above.
5. `cargo test && cargo clippy` clean.
6. `cargo run -- render-fixtures` — zero diff.

**Non-goal.** Don't change scene-record field semantics beyond
adding `kind: MaterialKind`. Don't introduce `MaterialId` (runtime
indirection) or a material registry — `MaterialKind` is a plain
enum. Don't introduce subkinds (`CutoutPlant`, `SunShadowFade`,
etc.) speculatively; add them when a fixture or parity fix
requires them. Don't expand `LayerOrdering` beyond the four rules
above unless a real layer needs a fifth.

---

### Phase 3 — Split scene assembly: `StaticSceneBuilder` + `LiveFrameBuilder`

**This is the load-bearing step.** The two entry points currently
flatten `WorldState` through divergent paths; pull both onto a
shared static builder, plus a separate live builder that the runtime
re-invokes per tick. The split mirrors the existing static/dynamic
cadence in the renderer's setter API (`set_static_overlays` is called
once at fixture load; `set_dynamic_sprites` is called per frame).
r1's single-builder framing erased that distinction; r2 preserves it.

**Change A.** Define the static contract in `src/scene/static.rs`:

```rust
pub struct StaticScene {
    pub terrain_sprites:    Vec<SpriteRecord>,
    pub static_sprites:     Vec<SpriteRecord>,    // walls, things, etc.
    pub edge_sprites:       Vec<EdgeSpriteInput>,
    pub colored_overlays:   Vec<ColoredMeshInput>,    // tagged by Layer
    pub textured_overlays:  Vec<TexturedMeshInput>,   // tagged by Layer
    pub sun_shadow:         Option<SunShadowParams>,
    pub initial_pawn_nodes: Vec<PawnNode>,             // baseline sprites; live path overrides per-pawn
}

pub fn build_static_scene(
    world: &WorldState,
    defs: &DefSet,
    assets: &mut AssetResolver,
) -> Result<StaticScene>;
```

`build_static_scene` calls the existing per-overlay functions
(`build_fog_overlay`, `build_snow_overlay`,
`build_shadow_overlays`, `build_lighting_overlay`, `compose_pawn`,
linking-edge resolution, terrain sprite layout) and merges results.
**It does not borrow `TextureRegistry`** — it produces records with
`SceneTexture::Asset` / `SceneTexture::Image`; the renderer ingests.

**Change B.** Define the live contract in `src/scene/live.rs`:

```rust
pub struct LiveFrameContext {
    pub tick:           u64,
    pub elapsed_secs:   f32,
    pub sun_dir:        Vec3,
    // future: animation phase, weather amplitude, etc.
}

pub struct LiveFrame {
    pub dynamic_sprites: Vec<SpriteRecord>,    // movement-interpolated pawns
    pub markers:         Vec<SpriteRecord>,    // hover, selection, path
    // future: per-frame effect overlays (splash, glow pulse)
}

pub fn build_live_frame(
    world:    &WorldState,
    static_:  &StaticScene,    // for handles into pre-uploaded textures
    ctx:      &LiveFrameContext,
) -> LiveFrame;
```

The `LiveFrameContext` is the new home for animation/time state —
created and advanced by the runtime tick loop (`src/runtime/v2/`),
threaded into `build_live_frame`. **`RenderState` is not touched in
this phase.** If a future feature shows that animation phase belongs
on world data rather than as a frame-context parameter, promote it
then; the parameter shape is the less invasive default.

`build_live_frame` references `&StaticScene` to reuse texture
handles minted during ingestion of the static scene (e.g. pawn body
textures). After ingestion, `StaticScene` records have been
canonicalized to `SceneTexture::Handle(...)` form; the live builder
copies handles for moving pawns rather than re-resolving paths.

**Change C.** Migrate `commands/fixture_cmd.rs` to:
1. Load fixture, build `WorldState` via `world_from_fixture`.
2. Call `build_static_scene(&world, &defs, &mut assets)` → `StaticScene`.
3. Hand to renderer via `Renderer::ingest_static(static_scene)` →
   the renderer uploads textures and stores handles.
4. Render once; screenshot.

**Change D.** Migrate `runtime/v2/render_bridge.rs` to:
1. At runtime startup, `build_static_scene` → `Renderer::ingest_static`
   (one-shot, identical to fixture path).
2. Per tick: runtime advances `WorldState` (pawn positions, path
   progress) and constructs a `LiveFrameContext`.
3. Per tick: `build_live_frame(&world, &static_scene, &ctx)` →
   `LiveFrame` → `Renderer::set_live_frame(live_frame)`.

The renderer's draw call combines the cached static handles with
the live frame's per-tick overrides. The existing render_bridge
behavior of "passthrough base list + override pawn entries" is
preserved in shape; movement-interpolated pawn `SpriteRecord`s
replace baseline pawn entries during the merge.

**Cadence summary:**

|                           | Static fixture entry | Live runtime entry |
|---|---|---|
| `build_static_scene`      | once at load         | once at startup    |
| `Renderer::ingest_static` | once at load         | once at startup    |
| `build_live_frame`        | not called           | per tick           |
| `Renderer::draw`          | once                 | per frame          |

**Verification.**
1. `rg "crate::renderer::(SpriteInstance|TextureId|FULL_UV_RECT)" src/runtime/ src/scene/ src/commands/`
   returns zero hits. All scene/runtime/command code goes through
   `crate::scene::*` types.
2. `cargo run -- render-fixtures` — zero diff. Static path must
   produce identical pixels.
3. Live-runtime smoke (manual): exercise
   `fixtures/v2/move_lane.ron` and
   `fixtures/v2/obstacle_pathing.ron` — pawn movement, hover,
   selection, path overlays, walls-above-pawns ordering all
   unchanged.
4. `cargo test && cargo clippy` clean.

**Non-goal.** Don't add dirty tracking, partial scene mutation, or
rebuild-on-change for static overlays. If a fixture mutation
mid-runtime needs to invalidate the static scene (e.g. wall built),
that's a separate concern — for now, static rebuild is a full
re-call of `build_static_scene` + `Renderer::ingest_static`, which
the runtime can trigger explicitly. Don't add `Tick()` to scene
records.

**Risk.** `runtime/v2/render_bridge.rs`'s movement interpolation
currently takes a base sprite list and edits pawn entries
(`src/runtime/v2/render_bridge.rs:17`). The new shape moves the
override into `LiveFrame::dynamic_sprites`, and the renderer's draw
step performs the merge (drop baseline pawn entries from
`StaticScene::initial_pawn_nodes` whose pawn ID appears in
`LiveFrame::dynamic_sprites`, then concatenate). Subtle stacking-order
shifts are the highest-risk regression and won't be caught by
`render-fixtures` (static-only). Mitigation: extend the fixture-render
harness to render fixed-tick snapshots of `move_lane.ron` /
`obstacle_pathing.ron`, or at minimum a manual checklist before
merging Phase 3.

---

### Phase 4 — Explicit `RenderPassStep` enum + named offscreen targets

**Change.** Replace `Renderer::render`'s implicit ordered draw
sequence (`src/renderer.rs:1449`) with an explicit pass list owned
by `FrameRenderer`. The list is keyed on `Layer`; pipeline routing
within each layer is keyed on `MaterialKind`.

```rust
// renderer/frame.rs
enum RenderPassStep {
    OffscreenTarget(OffscreenId),     // begin/clear that target
    LayerBatch(Layer),                 // drain all records tagged with this Layer,
                                       // sorted per Layer::ordering(),
                                       // dispatched to pipeline by MaterialKind
    Present,                            // begin/end swapchain pass
}

pub struct OffscreenTargets {
    pub water_depth: OffscreenTarget,   // R16Float, viewport-sized
    // future: fog_of_war, glow_blur, etc.
}

pub struct OffscreenTarget {
    pub format: wgpu::TextureFormat,
    pub texture: wgpu::Texture,
    pub view:    wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

impl OffscreenTarget {
    pub fn resize(&mut self, gpu: &GpuContext, size: (u32, u32));
}

// Within FrameRenderer when handling LayerBatch:
fn dispatch(&self, kind: MaterialKind, pass: &mut wgpu::RenderPass) -> &wgpu::RenderPipeline {
    match kind {
        MaterialKind::Cutout        => &self.pipelines.cutout,
        MaterialKind::Terrain       => &self.pipelines.terrain,
        MaterialKind::TerrainEdge   => &self.pipelines.terrain_edge,
        MaterialKind::TerrainWater  => &self.pipelines.terrain_water,
        MaterialKind::WaterDepth    => &self.pipelines.water_depth,
        MaterialKind::LightOverlay  => &self.pipelines.light_overlay,
        MaterialKind::EdgeShadow    => &self.pipelines.edge_shadow,
        MaterialKind::SunShadow     => &self.pipelines.sun_shadow,
        MaterialKind::FogOfWar      => &self.pipelines.fog_of_war,
        MaterialKind::Snow          => &self.pipelines.snow,
        MaterialKind::Transparent   => &self.pipelines.transparent,
        MaterialKind::SolidColor    => &self.pipelines.solid_color,
    }
}
```

`FrameRenderer::execute` walks a fixed `&[RenderPassStep]` list and
matches each step to its draw work. The pass list itself is a
`const`-like value built in `FrameRenderer::new`; it is **not**
runtime-extensible (per non-goal in §2).

**`Layer` and `MaterialKind` are independent axes.** `Layer`
determines *when* (which pass slot, which within-layer sort rule);
`MaterialKind` determines *which pipeline + blend state*. The
renderer dispatches on each independently — no `(Layer, MaterialKind)`
sparse routing table. A `MaterialKind::TerrainWater` record drawn in
`Layer::Terrain` and (hypothetically) in `Layer::AfterTerrain` both
go through the same water surface pipeline, but in different pass
slots. Keeping the axes orthogonal avoids a registry of valid
combinations.

**Concrete enum, not trait objects.** wgpu render passes hold
overlapping borrows on bind groups, buffers, and pipelines bounded by
the encoder's lifetime. A `Vec<Box<dyn Pass>>` with a `&mut
RenderPass` argument fights the borrow checker; the concrete enum +
match gives the same explicit ordering with no plugin-extensibility
cost (which we don't need).

**Resize behavior.** `Renderer::resize` walks `OffscreenTargets` and
calls `resize` on each — eliminates the special case water has today.

**Verification.**
1. `Renderer::render`'s body is now ~20 lines: build the encoder,
   walk the pass list, submit. All draw-call detail lives in
   `FrameRenderer` step handlers.
2. Add a unit test asserting the pass-step list is the expected
   sequence (purely structural — no GPU needed).
3. Add a unit test asserting `MaterialKind` → pipeline dispatch
   covers every variant (exhaustive `match` enforced by the
   compiler; the test guards against silent fallthroughs).
4. Add a unit test asserting `Layer::ordering()` returns the
   expected `LayerOrdering` for every variant.
5. `cargo run -- render-fixtures` — zero diff.
6. Manually resize the window with the v2 runtime running; no
   wgpu validation errors about stale offscreen attachment sizes.

**Non-goal.** Don't generalize to a frame graph. Don't introduce
resource aliasing, automatic barriers, or pass dependencies. The
list is a hand-written sequence; that's the right abstraction at 8
passes. Don't introduce `(Layer, MaterialKind)` sparse routing
tables — orthogonal axes only.

---

## 5. What we are explicitly *not* doing as part of this plan

- **`commands/fixture_cmd.rs` cleanup beyond scene-assembly
  extraction.** At 708 LOC, that command also handles RON loading,
  def resolution, runtime bootstrap, screenshot output, launch-spec
  assembly. Phase 3 removes the scene-building middle. The remaining
  responsibilities are command-layer concerns and stay where they
  are. A separate plan can address that command's shape if it becomes
  painful.
- **A neutral `RenderScene` IR with `MaterialId` indirection over
  existing CPU records.** The current input types are already neutral
  CPU data; wrapping them in another layer with no second backend or
  consumer is symmetry without payoff.
- **Trait-objectifying `SceneBuilder`.** It's a single concrete
  builder. If a debug / wireframe / minimap variant ever appears,
  trait-ify then.
- **Porting individual RimWorld shader files 1:1.** Goal is parity
  via *role naming* and *blend/render-state correctness*, not
  literal shader transliteration. Multiple `MaterialKind` variants
  may share one WGSL module today (e.g. `Cutout`, `Terrain`,
  `Transparent` could initially all dispatch to a generic textured
  sprite shader with different blend states). Split a shader only
  when behavior diverges enough to justify the file. Source
  reference for behavior is
  `~/rimworld-shader-extract/assetripper-1.3.5-decompile-export/`
  for ShaderLab + render state, and
  `assetripper-1.3.5-disassembly-export/` for Metal subprogram
  inspection — never vendor extracted code.

## 6. Risks

### 6.1 Visual parity across the live runtime path

`runtime/v2/render_bridge.rs` does pawn-movement interpolation by
mutating a sprite list. Migrating that into `SceneBuilder` (via
`RenderState` interpolated positions) is the highest-risk move in
Phase 3. A subtle layering or z-order shift will not be caught by
`render-fixtures` because the static fixture path doesn't exercise
movement. Mitigation: extend the fixture-render harness to also
render a fixed-tick snapshot of `move_lane.ron` /
`obstacle_pathing.ron` at known ticks, or at minimum a manual
checklist (walk-pawn, wall-rendering above pawn, overlay layers above
walls) before merging Phase 3.

### 6.2 Phase 0 inflating renderer module imports everywhere

A naive split that touches every `use crate::renderer::Foo` site in
the tree creates a noisy diff. Mitigation: `pub use` the moved types
from `renderer/mod.rs` so external imports keep working; only update
import paths in Phase 2 when types actually leave the `renderer`
module.

### 6.3 Renderer ingestion of `SceneTexture::Asset` paths

Phase 2 makes the renderer responsible for resolving asset paths in
scene records. This means the renderer borrows the `AssetResolver`
at `ingest` time. That dependency is asymmetric (renderer reads from
AssetResolver; AssetResolver doesn't know about the renderer) and
type-shallow — only the ingest step touches it, not the draw loop.
Acceptable. Document in `Renderer::ingest_static` that the
AssetResolver borrow is short-lived and write-free against scene
records.

### 6.4 Static/live merge correctness

`Renderer::draw` combines `StaticScene` baseline pawns with
`LiveFrame::dynamic_sprites`. The merge rule (drop static pawn
entries whose ID is overridden by a live entry, then concatenate) is
where the existing `render_bridge` behavior must be preserved. A
mismatched key, a missed override, or an ordering inversion produces
ghost-pawn or stacking-order regressions. Mitigation: unit-test the
merge function with synthetic StaticScene + LiveFrame inputs;
require pawn-ID coverage in test fixtures.

### 6.5 `SceneTexture::Image` per-frame allocation cost

Live-path scene records may carry `Arc<RgbaImage>` on rare
just-resolved textures. `Arc` clones are cheap but the renderer's
content-hash dedup must hash on first sight only and then look up by
`Arc::as_ptr()` or a stored hash to avoid re-hashing per frame.
Mitigation: cache `(*const u8, len)` → `TextureHandle` in the
registry alongside the content-hash key; lookup is then O(1) for
repeat references.

### 6.6 Parity fixes intentionally move pixels

The Phase 1 follow-up parity fixes (LightOverlay blend mode,
SunShadowFade pipeline) and any future material-family corrections
will produce different fixture screenshots. The zero-diff
`render-fixtures` gate and intentional parity fixes fight each
other unless the workflow is explicit. Mitigation: re-baseline
affected fixture renders in the same commit as the parity fix,
with a commit message that names the visual change ("LightOverlay
now uses DstColor SrcColor blend; affected fixtures: …"). Do not
bundle unrelated work into a re-baseline commit. The reviewer
should be able to look at each baseline change and map it to a
single intentional fix.

### 6.7 `MaterialKind` migration churn in Phase 2

Adding a `kind: MaterialKind` field to every scene record forces
every producer (~10 sites in `commands/`, plus pawn composer and
runtime bridge) to declare a kind in one commit. The risk is
miscategorisation — e.g. tagging a transparent overlay as
`Cutout`. Mitigation: walk the producer table in Phase 2 §"Add
`scene::MaterialKind`" with a reviewer; cross-check against the
existing pipeline a producer already routes through. Compiler
helps: exhaustive `match` on `MaterialKind` in the renderer's
dispatch will fail if a producer specifies a kind the pipeline set
doesn't yet handle.

## 7. Testing Strategy

- **`cargo run -- render-fixtures` zero-diff is the primary
  regression signal.** Per `AGENTS.md`, fixture renders must be
  re-rendered after each commit. Phases 0–2 must produce zero diff;
  Phase 3 must produce zero diff for the static path; Phase 4 must
  produce zero diff.
- **Unit tests at module boundaries.** Phase 0: none new
  (mechanical). Phase 1: `TextureRegistry` dedup test if feasible
  without device. Phase 2: (a) import-graph assertion that
  `scene/` has zero `crate::renderer::` imports (verify by
  temporarily commenting out `pub mod renderer;` and confirming
  `scene/` type-checks); (b) `Layer::ordering()` returns expected
  `LayerOrdering` for every variant; (c) every producer in the
  Phase 2 producer table assigns the documented `MaterialKind`
  (spot-check assertion).
  Phase 3: (a) `build_static_scene` smoke test against a minimal
  `WorldState`; (b) static/live merge unit test with synthetic
  inputs covering pawn-ID override, ordering, and z-layer
  preservation; (c) `LiveFrameContext` advances independently of
  `WorldState` mutation.
  Phase 4: (a) pass-list structural test; (b) `MaterialKind` →
  pipeline dispatch covers every variant (compiler-enforced
  exhaustive match guards against silent fallthroughs).
- **Live-runtime manual smoke after Phase 3.** Walk-a-pawn fixture
  with v2 runtime; confirm movement, animation, fog updates render
  identically.
- **Lint policy unchanged.** Per `AGENTS.md`: no `#[allow(clippy::*)]`
  per-item. If clippy fires on the new module shape (e.g.
  `too_many_arguments` on `SceneBuilder::new`), restructure rather
  than allow.

## 8. Open Questions

- **Does `SpriteParams` belong in `scene/` or `renderer/`?**
  (`src/renderer.rs:2384`.) It's neutral CPU data shaped like a
  sprite description, but it carries some renderer-shaped concerns
  (UV rect, z-order). Default: `scene/`. Revisit if it picks up
  wgpu-shaped fields.
- **Does `EdgeSpriteInput` + `EdgeFan` topology belong in `scene/`
  or `renderer/`?** (`src/renderer.rs:2231,2257`.) The fan vertex
  layout is GPU-shaped; the input list is scene-shaped. Probably
  split: `EdgeSpriteInput` (scene) carries cell positions + edge
  type; `EdgeFan` topology (renderer) is built from it during draw.
  Decide in Phase 2.
- **Where does the screenshot-readback path live after Phase 0?**
  Almost certainly `renderer/screenshot.rs` (it's GPU-shaped — copy
  texture to buffer, map, decode), called by `Renderer::render`
  when options request it. Confirm during Phase 0.
- **`build_static_scene` and `build_live_frame` as free functions or
  struct builders?** r2 specifies free functions for both. Promote
  to struct builders only if cross-call caching emerges.
- **Should `LiveFrameContext` carry a reference to the previous
  frame's context (for delta computation, e.g. interpolation between
  ticks)?** Probably not — the runtime can carry that state itself
  and bake it into `WorldState` mutation before calling
  `build_live_frame`. Revisit if a feature requires explicit
  inter-tick deltas at the scene boundary.
- **Where does the static-scene rebuild trigger live for the live
  runtime?** When a wall is built / fog reveals / lighting changes,
  the static scene is stale. Default: runtime explicitly re-calls
  `build_static_scene` + `Renderer::ingest_static` on those events.
  No automatic invalidation. If invalidation patterns repeat, factor
  into a `dirty: bool` flag on the runtime side, not the scene side.
- **Should `MaterialKind` carry blend-state metadata, or is the
  dispatch table the single source of truth?** Default: dispatch
  table only. The `match` in `FrameRenderer::dispatch` knows blend
  state via the pipeline it returns. If multiple `MaterialKind`s
  share a pipeline but need different blend states, that's a sign
  the pipeline split was too coarse and the answer is to add a
  pipeline, not metadata.
- **When does it pay to split a `MaterialKind` variant into
  subkinds?** Trigger: a fixture or parity bug shows current
  behavior is wrong because two records of the same kind need
  different shader inputs or render state (e.g. plant sway vs.
  static cutout). Don't split speculatively — wait for the
  forcing function.
- **Are there `MaterialKind` candidates that current code doesn't
  touch but should be added in Phase 2 anyway?** Default: no. The
  12-variant set is bounded by current call sites; expansion comes
  from new features or parity work.
