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

Both `commands/fixture_cmd.rs` and `runtime/v2` now flow through the
single seam `crate::scene::builder::build_scene`. The reusable
scene-assembly pieces moved out of `commands/` so the new seam is
genuinely neutral — `scene/` no longer imports from `commands/` and
`commands` is a pure caller via `LaunchSpec`.

What landed:

- **Builder has a symmetric two-step contract.**
  `compute_pawn_profiles(defs, assets, world) -> HashMap<id, profile>`
  runs once at scene init; `build_scene(defs, world, &profiles,
  compose_config) -> Scene` runs per frame. Pawns missing from the
  profiles map are an error, not a silent fallback. No `AssetResolver`
  parameter — scene textures are path-backed and resolve to handles at
  renderer ingest.
- **`commands::DefSet` moved to `defs::DefSet`.** It's a borrow-bundle
  of def hashmaps; the def module is its natural home. `commands::DefSet`
  remains as a re-export for callers that already import it.
- **Pawn-render directional/apparel helpers moved to
  `pawn::render_input`.** `resolve_directional_tex_path`,
  `apparel_worn_data_for_facing`, `build_apparel_tex_path`,
  `build_full_apparel_layer_override`, `map_explicit_skip_flags` —
  these compute fields that go into `PawnRenderInput`, not
  command-shaped logic.
- **Linking-sprite emitters moved to `scene::linking_sprites`** and the
  full overlay tree (fog/snow/shadow/lighting + their helpers
  `solid_mesh`, `sky_shadow`, `glow_grid`) moved into
  `scene::overlays`. `commands/` now only holds CLI-shaped modules
  (`fixture_cmd`, `render_cmd`, `debug_cmd`, `common`).
- **Drop dead parameters.** `data_dir` was unused in
  `emit_linked_thing_sprites` / `emit_terrain_edge_sprites` (both had
  `_data_dir`); `_asset_resolver` was unused in `build_fog_overlays` /
  `build_snow_overlays`. All gone, and `build_scene`'s signature
  shrinks accordingly.
- **`FrameContext` omitted entirely** for now. Plan-v3 listed it as a
  forward seam, but with no current reader the parameter would be
  silenced under `_`. Reintroduce when the first render-only frame
  field arrives (sub-tick interpolation alpha, animation phase,
  override sun direction for screenshots). The constraint *render-only
  state lives on the parameter, not `WorldState`* still applies when
  the type comes back.
- **Skip launch-time `dynamic_sprites` for runtime-driven fixtures.**
  The first redraw rebuilds them from `runtime.build_scene` anyway, so
  the fixture-time path no longer wastes work resolving textures for
  sprites that get immediately overwritten.
- **`apply_interaction_overlays`** (was `compose_dynamic_sprites`)
  inlined into `runtime/v2/mod.rs`. Pawn composition moved into
  `build_scene`, so the function is now path/hover/selection markers
  only — its name now matches its job.
- **`validate_layer_ownership` deleted.** The function reverse-
  engineered layer from `def_name` string prefixes; the source of
  truth is which Vec the sprite sits in, so the check was redundant.
- **Borrow gymnastics in viewer redraw** replaced with a free
  `populate_dynamic_records` function that accepts split borrows
  (`&mut Vec, &mut Renderer, &mut AssetResolver`) — drops the
  `renderer.take()` / `Some(renderer)` dance.
- **`TextureRegistry` methods take `&Device, &Queue` refs** instead of
  `&GpuContext`. The registry never touched the surface; threading
  only what it needs is a small architectural win and makes the
  headless wgpu test trivial.

Test gates closed:

- **`TextureRegistry::upload_count`** counter increments only when
  `register_texture` allocates a new wgpu texture (cache miss).
- **`resolve_texture_skips_fetch_and_upload_on_cache_hit`** drives the
  full `resolve_texture` path with a counted fetch closure standing in
  for `AssetResolver`. Asserts the fetch runs at most once per
  `SceneTexture` and each fetch corresponds to exactly one upload.
  This is the production-shape invariant for the per-frame
  no-double-upload gate.
- **`register_texture_dedupes_by_image_bytes`** covers the content-keyed
  dedupe inside `register_texture` (e.g. when two distinct
  `SceneTexture`s resolve to the same image bytes).
- **`V2Runtime::pawn_node_count()`** runs the same compose pipeline as
  `build_scene` without needing an `AssetResolver` or GPU. Restores the
  coverage previously implicit in the deleted
  `V2FrameOutput::pawn_nodes` assertion.

### Deferred from Commit 3

Each has a trigger so the followup doesn't rot.

- **`OwnedSceneDefs` Arc-share.** `V2Runtime` clones every def hashmap
  at construction and re-borrows them as a `DefSet<'_>` per frame. One
  clone, not per-tick — tolerable but wasteful. **Trigger:** `DefSet`
  construction or runtime setup shows up in a startup profile, OR a
  second `V2Runtime` instance per process becomes a feature. Real fix
  is `Arc<HashMap<...>>` inside `DefSet` (or `Arc<DefSet<'static>>`) so
  the runtime borrows cheaply instead of cloning.
- **`SceneSprite` `String` → `Arc<str>` (or small enum).** `def_name:
  String` and `node_id: Option<String>` allocate per sprite per frame.
  At a few pawns it's noise; at realistic populations it's measurable.
  **Trigger:** per-frame pawn-node count exceeds ~500 in a real
  fixture, OR allocation shows up in a frame profile. Push `Arc<str>`
  upstream through `PawnNode::id` at the same time so the saving is
  end-to-end.
- **`FrameContext` reintroduction.** Currently absent. **Trigger:**
  the first render-only frame field needed by `build_scene` — most
  likely sub-tick interpolation alpha when smooth pawn movement
  between integer cells lands. Reintroduce with just the field that
  has a reader, not the full plan-v3 shape.

### Open from earlier commits

- **Graphic shadows still piggyback on `MaterialKind::SunShadow`.**
  RimWorld treats graphic shadow data separately from projected
  static sun shadows. **Trigger:** adding material subkinds, or any
  shadow pipeline parity work that needs to distinguish the two.

## After Commit 4 — `RenderPassStep` + offscreen targets

_(empty until landed)_
