# Followups — Renderer Engine Boundary

Per-commit notes captured *after* each commit in `plan-v3.md` lands.
This file is intentionally empty at start; fill in incrementally as
the work ships, not in advance. Each section answers two questions:
**what did we notice in this commit that we couldn't have known in
advance?** and **what does that change for the next commit?**

If a noticed item is concrete enough to be its own deferred work
item, mirror it to `plans/BACKLOG.md` rather than letting it accrete
here.

## After Commit 1 — Renderer module split

Landed in `bd60cb7`. Acceptance gates passed (zero fixture diff, fmt/clippy/test clean, `Renderer::new` is 32 lines). Items noticed during review, address opportunistically as Commit 2 reshuffles types:

- **Move batch types from `renderer/mod.rs` to `renderer/frame.rs`.** `SpriteBatch`, `EdgeSpriteBatch`, `ColoredMeshBatch`, `TexturedMeshBatch`, `SunShadowBatch`, and the `GroupedSpriteInstances` alias (mod.rs:44-84) are `pub(crate)` per-frame state held by `FrameRenderer`. They're in mod.rs only because they reference public types; let `frame.rs` import those instead.
- **Move `multiply_overlay_blend_state()` from `renderer/mod.rs` to `renderer/pipelines.rs`** as a private helper. Only consumer is `pipelines.rs`.
- **Resolve `#[allow(dead_code)]` on `TextureRegistry::texture_images` (textures.rs:21-22).** Per AGENTS.md "dead parameters should be removed, not silenced" — applies to unused struct fields. Either delete the field (it's populated by `register_texture` but never read) or document why it must stay.
- **`PipelineSet` owns more than pipelines.** It also holds `noise_bind_group`, `water_depth_sampler`, `water_ramps_bind_group`, and the unit-quad `vertex_buffer` / `index_buffer`. Layouts and samplers legitimately belong (pipelines need them at construction); the bind groups and unit-quad buffers feel more like `FrameRenderer` concerns. Tighten when Commit 4 formalizes offscreen targets — not urgent now.
- **`Vertex`, `SunShadowUniform` in mod.rs are GPU-shaped** and could live in their consuming submodule (`pipelines.rs` or `frame.rs`). Will be addressed naturally as Commit 2 sorts types between `scene/` and `renderer/`.

## After Commit 2 — `src/scene/` extraction

First slice landed in `e820d9a`. It extracted the neutral `scene` records,
renamed `TextureId` to `TextureHandle`, moved GPU byte-layout descriptors back
to renderer-owned helpers, and replaced `is_water` / `is_terrain` routing with
`MaterialKind`.

- **Path-backed scene textures were intentionally deferred, but they block
  Commit 3.** `SpriteInput`, `TexturedMeshInput`, and `EdgeSpriteInput` still
  carry `RgbaImage`, and overlay builders still resolve material backing
  textures directly. That was kept out of `e820d9a` to avoid mixing the
  mechanical type extraction with `LaunchSpec` / `viewer` / `TextureRegistry`
  plumbing. Do this before the shared `build_scene` work: introduce a single
  path-backed scene texture form, move fog/snow material texture resolution to
  renderer ingest, and add the path-cache test required by `plan-v3.md`.
- **Graphic shadows need explicit material classification.** They currently
  use `MaterialKind::SunShadow` only because Commit 2's material table did not
  name graphic shadows separately. RimWorld treats graphic shadow data
  separately from projected static sun shadows; revisit when adding material
  subkinds or shadow pipeline parity.

## After Commit 3 — Single `build_scene`

Initial slice landed in `ace4195` (single seam, both paths through
`crate::scene::builder::build_scene`). That commit shipped two unprompted
scope expansions — feature-flagged `SceneBuildOptions` and a
`SceneBuildOutput` that bundled pawn profiles with the scene — which
were unwound in `f312728` along with several smaller cleanups. Test gates
were closed in `29f4107`.

Items addressed in `f312728`:

- Split `compute_pawn_profiles` from `build_scene`. Builder now has a
  symmetric two-step contract: profiles once at scene init, then
  `build_scene` per frame with the resulting `&HashMap`. Pawns missing
  from the map are an error, not a silent fallback.
- Drop dead parameters across the builder: `data_dir` from `build_scene`
  and the linking helpers, `_asset_resolver` from `build_fog_overlays`
  and `build_snow_overlays` (path-backed scene textures resolve at
  renderer ingest, so the resolver isn't needed during scene assembly).
- `FrameContext` removed entirely. Plan-v3 listed it as a forward seam,
  but the parameter was unused (`_frame: &FrameContext`) and the
  underscore prefix violates "dead parameters should be removed, not
  silenced." Reintroduce when the first render-only frame field arrives
  (sub-tick interpolation alpha, animation phase, override sun direction
  for screenshots) — at which point the *constraint* still holds:
  render-only state lives on the parameter, never on `WorldState`.
- Skip launch-time `dynamic_sprites` for runtime-driven fixtures. The
  first redraw rebuilds them from `runtime.build_scene` anyway, so the
  fixture-time path no longer wastes work resolving textures for sprites
  that get immediately overwritten.
- `compose_dynamic_sprites` renamed to `apply_interaction_overlays` and
  inlined into `runtime/v2/mod.rs`. Pawn composition moved into
  `build_scene` in Commit 3, leaving the function as a 1-file vestige
  with a misleading name.
- `validate_layer_ownership` deleted. The function reverse-engineered
  layer from `def_name` string prefixes; the source of truth (which Vec
  the sprite sits in) made the check redundant theater.
- Borrow gymnastics in viewer redraw replaced with a free
  `populate_dynamic_records` function that takes split borrows.

Items addressed in `29f4107`:

- `TextureRegistry::upload_count` counter, incremented only when
  `register_texture` allocates a new wgpu texture. Headless wgpu test
  asserts upload count after two same-bytes uploads is 1 (and bumps to 2
  for distinct bytes, proving the dedupe is content-keyed). Closes the
  per-frame "no double upload" gate.
- `V2Runtime::pawn_node_count()` runs the same compose pipeline as
  `build_scene` without needing an `AssetResolver` or GPU. Restores the
  coverage previously implicit in the deleted
  `V2FrameOutput::pawn_nodes` assertion.
- `TextureRegistry` methods now take `&wgpu::Device, &wgpu::Queue`
  refs instead of `&GpuContext`. The registry never touched the surface;
  threading only what it needs makes the headless test trivial and is a
  small architectural improvement on its own.

### Deferred from Commit 3

These were intentionally not addressed in 3a/3b. Each has a trigger so
the followup doesn't rot.

- **`OwnedSceneDefs` Arc-share.** `V2Runtime` clones every def hashmap
  at construction and re-borrows them as a `DefSet<'_>` per frame. One
  clone, not per-tick — tolerable but wasteful. **Trigger:** `DefSet`
  construction or runtime setup shows up in a startup profile, OR a
  second `V2Runtime` instance per process becomes a feature. Real fix is
  `Arc<HashMap<...>>` inside `DefSet` (or `Arc<DefSet<'static>>`) so the
  runtime borrows cheaply instead of cloning.
- **`SceneSprite` `String` → `Arc<str>` (or small enum).** `def_name:
  String` and `node_id: Option<String>` allocate per sprite per frame.
  At a few pawns it's noise; at realistic populations it's measurable.
  **Trigger:** per-frame pawn-node count exceeds ~500 in a real fixture,
  OR allocation shows up in a frame profile. Push `Arc<str>` upstream
  through `PawnNode::id` at the same time so the saving is end-to-end.
- **`FrameContext` reintroduction.** Currently absent. **Trigger:** the
  first render-only frame field needed by `build_scene` — most likely
  sub-tick interpolation alpha when smooth pawn movement between integer
  cells lands. At that point reintroduce `FrameContext` with just the
  field that has a reader, not the full plan-v3 shape. The constraint
  *render-only state lives on the parameter, not WorldState* still
  applies.

### Open from earlier commits

- **Graphic shadows still piggyback on `MaterialKind::SunShadow`.**
  RimWorld treats graphic shadow data separately from projected static
  sun shadows. **Trigger:** adding material subkinds, or any shadow
  pipeline parity work that needs to distinguish the two.

## After Commit 4 — `RenderPassStep` + offscreen targets

_(empty until landed)_
