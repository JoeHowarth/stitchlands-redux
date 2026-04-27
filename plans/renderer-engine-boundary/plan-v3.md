# Renderer Module Split + Scene/Runtime Seam

Make the boundary between scene assembly and the renderer visible
and tested, so the static fixture command (`commands/fixture_cmd.rs`)
and the live v2 runtime (`runtime/v2/render_bridge.rs`) stop
producing draw data through divergent paths. The goal is *not* to
build an engine; it is to give the existing two consumers a single
named seam they both go through.

This is a medium refactor scoped to four shippable commits, each of
which leaves the codebase better than it found it. The plan is
valuable even if only the first one lands.

## Why now

- `src/renderer.rs` is one 2,408-line struct doing four jobs:
  device/surface ownership, pipeline construction, texture
  upload/dedup, and frame execution. Adding water (the most recent
  multi-pipeline feature) required surgery in `Renderer::new` and
  `Renderer::render` simultaneously.
- `OverlayPass` (`src/renderer.rs:2081`) is a renderer-internal enum
  imported by every overlay builder under `src/commands/`
  (`fog_overlay.rs:5`, `snow_overlay.rs:4`, `lighting_overlay.rs:7`,
  `shadow_overlay.rs:9`, `solid_overlay_mesh.rs:3`,
  `linking_sprites.rs:14`, `overlays.rs:7`, `mod.rs:44-46`).
- The v2 runtime (`runtime/v2/render_bridge.rs:6`) imports
  `crate::renderer::{FULL_UV_RECT, SpriteInstance, SpriteParams,
  TextureId}` directly and produces draw records by mutating a
  `&[SpriteInstance]` base list. The fixture command builds the
  same kind of records through a different path. As live-runtime
  features grow, these will drift further.
- `is_water` / `is_terrain` booleans route sprites in
  `src/renderer.rs:1524-1550` instead of named material data.
- The water depth pass (`src/renderer.rs:1473-1506`) hand-wires
  its RT, view, format, and bind group on the `Renderer` struct
  directly. A second offscreen feature (fog of war, glow) would
  duplicate that wiring rather than reuse it.

## Target shape

```
src/renderer/        ← GPU machinery, ingests scene records
  gpu_context.rs       device, queue, surface, config
  textures.rs          TextureRegistry: upload, dedup, bind groups
  pipelines.rs         PipelineSet: every wgpu::RenderPipeline
  frame.rs             FrameRenderer: per-frame batches, pass execution
  mod.rs               Renderer = thin coordinator over the four

src/scene/           ← neutral CPU records, no wgpu deps
  layer.rs             Layer enum + LayerOrdering rule per layer
  material.rs          MaterialKind enum (12 variants, initial set)
  records.rs           SpriteRecord, ColoredMeshInput, etc.
  builder.rs           build_scene(world, defs, assets) -> Scene

src/commands/        ← unchanged in shape; imports scene::*
src/runtime/v2/      ← unchanged in shape; imports scene::*
```

`Renderer` exposes one ingestion entry point: `Renderer::draw(scene:
&Scene)`. It walks records, resolves textures via the registry,
dispatches to pipelines by `MaterialKind`, applies per-layer
ordering, and submits passes in a fixed sequence.

`Layer` and `MaterialKind` are independent in principle: layer
decides *when*, material decides *which pipeline*. In practice the
combinations are sparse (TerrainEdge only appears in Terrain;
FogOfWar only in AfterDynamic). The renderer doesn't enforce
combinations — it just dispatches — but the producer table below
documents what actually occurs.

## Sequence

Four commits. Each is independently shippable. Stop-here criteria
mark the points where landing nothing further still leaves a real
improvement.

### Commit 1 — Split `renderer.rs` into `renderer/` with four owners

Crack open `renderer.rs` once and give it the right shape rather
than two-stepping through a mechanical move + later restructure.

- New module `renderer/` with `gpu_context.rs`, `textures.rs`,
  `pipelines.rs`, `frame.rs`, `camera.rs`, `screenshot.rs`, `mod.rs`.
- `Renderer` becomes `{ gpu, textures, pipelines, frame, camera }`.
- Construction order: `GpuContext` → `TextureRegistry` (owns the
  shared sprite BGL) → `PipelineSet::build(&gpu, &textures)` →
  `FrameRenderer`.
- `pub use` from `renderer/mod.rs` so external callers
  (`commands/`, `runtime/`, `viewer.rs`) see no import path
  changes.
- Public API of `Renderer` (setters called by `viewer.rs` and
  `commands/fixture_cmd.rs`) is unchanged; setters delegate to
  the inner component.

**Done when**: `cargo run -- render-fixtures` produces zero diff;
`cargo test && cargo clippy` clean; `Renderer::new` is < 50 lines
and reads as four constructor calls.

**Stop here if**: the four-way split makes adding new pipelines
and offscreen targets easy enough that the rest of this plan
isn't urgent.

### Commit 2 — Extract `src/scene/` with `Layer` and `MaterialKind`

Move the neutral scene types out of `renderer/` and name the
material role of every record.

- New `src/scene/` with `Layer`, `LayerOrdering`, `MaterialKind`,
  `SpriteRecord` (renamed from `SpriteInstance`), `ColoredMeshInput`,
  `TexturedMeshInput`, `EdgeSpriteInput`, `OverlayBlendMode`,
  `SpriteParams`, `SunShadowParams`. Vertex byte layouts stay
  renderer-side as free functions referencing the scene structs.
- `TextureId` → `scene::TextureHandle`. Scene records carry
  `Arc<str>` asset paths (a single `SceneTexture` form). The
  renderer caches `path → TextureHandle` in `TextureRegistry`:
  first sight of a path → resolve via `AssetResolver` → upload →
  mint handle → store. Subsequent sightings → O(1) map lookup, no
  upload.
- **Behavior change for existing overlay builders.**
  `commands/fog_overlay.rs`, `commands/snow_overlay.rs`, and any
  other builder that today calls `assets.resolve_texture(...)` to
  fetch a material backing must stop doing so. They pass the asset
  path string (e.g. `"Misc/FogOfWar"`, `"Other/Snow"`) into the
  scene record's texture field. Resolution moves to renderer ingest
  time. Builders may still borrow `&AssetResolver` for non-texture
  lookups; they just don't decode bytes for material textures
  anymore.
- `OverlayPass` → `scene::Layer` with **all seven slots first-class**:
  `BeforeWorld`, `Terrain`, `AfterTerrain`, `StaticThings`,
  `AfterStatic`, `Dynamic`, `AfterDynamic`. World batches are no
  longer implicit between overlay slots.
- `LayerOrdering` enum names the within-layer sort rule:
  `ByTerrainPrecedence` (Terrain), `ByAltitudeThenZ`
  (StaticThings, Dynamic), `InsertionOrder` (overlay slots),
  `Explicit` (markers). `Layer::ordering()` returns it.
- `MaterialKind` (12 variants, initial set):

  ```
  Cutout, Terrain, TerrainEdge, TerrainWater, WaterDepth,
  LightOverlay, EdgeShadow, SunShadow,
  FogOfWar, Snow, Transparent, SolidColor
  ```

  Subkinds (`CutoutPlant`, `SunShadowFade`, etc.) deferred until a
  fixture or parity fix forces them.
- Add `kind: MaterialKind` to every scene record. Migrate
  producers per the table below.
- Drop `is_water` / `is_terrain` booleans; replace with
  `MaterialKind` matches.

**Producer → MaterialKind**

| Producer                                                       | MaterialKind          |
|---------------------------------------------------------------|-----------------------|
| `commands/fog_overlay.rs`                                     | `FogOfWar`            |
| `commands/snow_overlay.rs`                                    | `Snow`                |
| `commands/lighting_overlay.rs`                                | `LightOverlay`        |
| `commands/shadow_overlay.rs` (sun shadows)                    | `SunShadow`           |
| `commands/shadow_overlay.rs` (edge shadows)                   | `EdgeShadow`          |
| `commands/linking_sprites.rs` (terrain edges)                 | `TerrainEdge`         |
| terrain sprite assembly                                       | `Terrain`             |
| thing/wall/static-sprite assembly, pawn composer (`pawn/`)    | `Cutout`              |
| water terrain (per `plans/water-rendering/`)                  | `TerrainWater`, `WaterDepth` |
| hover/selection markers, debug overlays                       | `SolidColor`          |
| transparent post-light overlays (when added)                  | `Transparent`         |

**Done when**: `rg "crate::renderer::" src/scene/` returns zero
hits; `rg "is_water\|is_terrain" src/renderer/` returns zero hits;
every producer in the table declares its kind; existing overlay
builders no longer call `assets.resolve_texture(...)` for material
backings (paths flow through scene records instead); a unit test
covers the `TextureRegistry` path-cache (re-uploading the same
path twice is a no-op on the second call); `cargo run --
render-fixtures` produces zero diff.

**Stop here if**: the boundary being visible was the main concern.
Commits 3–4 are about deduplicating the two assemblers, which is
useful but optional.

### Commit 3 — Single `build_scene` shared by both entry points

Both the fixture command and the live runtime call one builder.
Animation/time state is a parameter, not stored on `RenderState`.

- `pub fn build_scene(world: &WorldState, defs: &DefSet, assets:
  &mut AssetResolver, ctx: &FrameContext) -> Result<Scene>`.
- `FrameContext { tick: u64, elapsed_secs: f32, sun_dir: Vec3 }`.
  Static fixture: `tick = 0`, neutral defaults. Live runtime:
  advanced per tick.
- `commands/fixture_cmd.rs` calls `build_scene` once at fixture
  load, hands `Scene` to renderer.
- `runtime/v2/render_bridge.rs` calls `build_scene` per tick after
  the runtime has updated existing pawn position fields on
  `WorldState` (whatever today's `tick_world` already mutates —
  e.g. `pawn.world_pos`).
- The interpolation that lives in `render_bridge.rs:13-77` today
  moves into the runtime's tick step, writing into the existing
  pawn position fields. Builder reads those fields and emits
  records; no "override list" parameter.
- **Do not add render-only interpolation state to `WorldState`.**
  If a render-only field is needed (e.g. a sub-tick alpha for
  smooth movement between integer cells), it lives in
  `FrameContext`, not `WorldState`. `WorldState` is simulation
  data; render-frame state is a parameter.

**Single-builder rationale.** Static-vs-live builders would split
prematurely; the only "live" thing today is pawn-position
interpolation. Rebuild-per-tick is fine until profiles say
otherwise. If terrain rebuild cost shows up at realistic map
sizes, split *then* — `build_scene` becomes
`build_static_scene` + `build_live_frame` mechanically because
the per-overlay functions already partition cleanly.

**Done when**: `rg "crate::renderer::(SpriteInstance|TextureId|FULL_UV_RECT)"
src/runtime/ src/commands/` returns zero hits; `cargo run --
render-fixtures` produces zero diff; manual smoke of
`fixtures/v2/move_lane.ron` and `fixtures/v2/obstacle_pathing.ron`
shows identical pawn movement, hover, selection, and z-ordering;
texture upload count after the first rebuilt frame is zero in the
live runtime path (i.e. the `TextureRegistry` path-cache is
working — verified by an instrumented counter or a unit-style
test on `Renderer::draw` calling itself twice with the same
`Scene` and asserting no second-pass uploads).

**Stop here if**: the seam is the value; the renderer's internal
pass shape isn't pressing.

### Commit 4 — Explicit `RenderPassStep` + named offscreen targets

Replace the implicit ordered draw sequence with a named list and
generalize the water-depth offscreen wiring.

- `enum RenderPassStep { OffscreenTarget(OffscreenId),
  LayerBatch(Layer), Present }` owned by `FrameRenderer`. Concrete
  enum, not `Vec<Box<dyn Pass>>` — wgpu lifetimes fight trait
  objects and there's no extensibility need.
- `OffscreenTargets { water_depth: OffscreenTarget, … }` with a
  uniform `OffscreenTarget::resize` so adding fog-of-war /
  glow-blur RTs doesn't require special-casing.
- `FrameRenderer::dispatch(MaterialKind) -> &RenderPipeline`
  reflects the *current* pipeline structure (today
  `LightOverlay`/`EdgeShadow`/`SolidColor` share a colored-overlay
  pipeline; the dispatch returns that pipeline for all three). One
  pipeline per `MaterialKind` is aspirational — split a pipeline
  when blend state / inputs actually diverge.
- `Renderer::render` body shrinks to ~20 lines: open encoder, walk
  pass list, submit.

**Done when**: `cargo run -- render-fixtures` produces zero diff;
adding a hypothetical second offscreen RT requires no changes
outside `frame.rs` + `pipelines.rs`.

**Stop here if**: ever. This is the most optional commit.

## Deferred

| Item | Trigger to revisit |
|------|--------------------|
| Separate engine crate | A second binary or external consumer |
| Generic `RenderScene` IR with `MaterialId` indirection | A second renderer backend |
| Static-vs-live builder split | Per-tick rebuild cost shows in a profile |
| `MaterialKind` subkinds (`CutoutPlant`, `SunShadowFade`, etc.) | A fixture or parity fix needs them |
| `SceneTexture::{Image\|Handle}` variants beyond `Arc<str>` paths | Procedural textures or pre-bound startup textures |
| Frame graph (resource aliasing, automatic barriers) | Hundreds of pipelines, not 8 |
| Dirty tracking / incremental scene mutation | Profile-driven |
| `commands/fixture_cmd.rs` cleanup beyond the `build_scene` call | Separate plan |
| `LightOverlay` blend mode + `SunShadowFade` pipeline parity fixes | Tracked separately; cheap once `PipelineSet` exists, no architectural dependency |

## Risks

**Visual parity in the live runtime.** `cargo run --
render-fixtures` only catches the static path. Movement, hover,
selection, and z-ordering regressions slip through. Mitigation:
extend the harness to render fixed-tick snapshots of `move_lane.ron`
and `obstacle_pathing.ron`, or commit a written smoke checklist
that gets run before merging Commit 3.

**Pawn ID stability assumption.** Commit 3's runtime path replaces
baseline pawn entries with movement-interpolated ones. This works
only if pawn entries carry a stable identifier. **Verify before
starting Commit 3** that today's `compose_pawn` output has a usable
pawn ID; if not, threading one through is a prerequisite, not part
of the commit.

**Refactor abandonment.** The biggest risk is hitting Commit 2 and
losing interest. Half-done refactors are worse than none. The
stop-here criteria are real — Commit 1 alone is shippable; Commit 2
alone is shippable. Don't start Commit 3 unless 1 and 2 are merged
and steady.

## Open questions

- **Does today's pawn render output carry a stable pawn ID?**
  Block on this before Commit 3.
- **Where does `SpriteParams` live — `scene/` or `renderer/`?** It
  carries z-order which feels renderer-shaped but is set by the
  caller. Default `scene/`; revisit if it picks up wgpu fields.
- **Do edge-fan vertex buffers split between `scene/` (input list)
  and `renderer/` (fan topology built at draw time)?** Decide
  during Commit 2 when the types move.
- **Should the static-scene rebuild trigger be implicit (dirty
  flag) or explicit (runtime calls `build_scene` again on world
  mutation)?** Explicit is simpler; only revisit if runtime code
  ends up sprinkling rebuild calls everywhere.

## References

- Renderer survey of current state:
  `src/renderer.rs:26-75` (struct), `:456-927` (`new`), `:1449`
  (`render`), `:1473-1506` (water depth pass), `:1524-1550`
  (boolean sprite routing), `:2081` (`OverlayPass`).
- RimWorld decompile: `~/rimworld-decompiled/MAP/INDEX.md`.
- RimWorld shader extracts:
  `~/rimworld-shader-extract/INDEX.md`. Reimplement behavior from
  observed render state; do not vendor extracted code.
- Companion plans: `plans/water-rendering/` (water terrain),
  `plans/fog-snow-overlays/` (overlay material backing).
