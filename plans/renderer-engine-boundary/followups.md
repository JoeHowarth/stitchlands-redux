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

_(empty until landed)_

## After Commit 4 — `RenderPassStep` + offscreen targets

_(empty until landed)_
