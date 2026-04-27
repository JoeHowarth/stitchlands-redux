# Renderer Engine Boundary Plan

## Status

Active. This plan is for refactoring the current rendering/application boundary
so the repo can support two first-class entry points without growing more
feature-specific renderer glue:

- static fixture rendering from config-described RON fixtures,
- a live running game/runtime where pawns move and animations, effects,
  overlays, lighting, water, and other visual state can change over time.

The goal is not to build a generic reusable game engine. The goal is to make
the current app feel engine-shaped where the repo already has real pressure:
two frame producers should feed one renderer through a shared frame assembly
contract.

## Current Local Shape

The codebase already has useful seams:

- `world/` owns pure world state and deterministic stepping primitives.
- `runtime/v2/` owns live interaction and per-frame runtime output.
- `interaction/` owns input state transitions and picking helpers.
- `assets/` resolves loose and packed RimWorld texture assets through an
  injectable resolver.
- `pawn/` composes pawn render nodes from data-oriented render inputs.
- `renderer.rs` accepts CPU-side sprite and mesh inputs; command code does not
  directly touch `wgpu`.

The main problem is concentrated in the frame assembly and renderer internals:

- `src/renderer.rs` is a large god module that owns surface/device setup,
  pipelines, bind group layouts, texture deduplication, batching, camera input,
  pass ordering, water offscreen targets, and screenshot readback.
- `src/commands/fixture_cmd.rs` loads fixtures, builds `WorldState`, resolves
  defs and textures, assembles terrain/thing/pawn draw data, builds static
  overlays, bootstraps runtime state, configures renderer launch options, and
  emits diagnostic logs in one command path.
- Renderer policy concepts leak outward. `OverlayPass` is imported by overlay
  builders, while sprite routing is currently encoded with `is_water` and
  `is_terrain` booleans.

## Target Shape

There should be two producers and one renderer backend:

```text
fixture render entry point
  -> load fixture, defs, and assets
  -> static frame assembly
  -> render frame input
  -> renderer

live game entry point
  -> runtime/world/interaction
  -> live frame assembly
  -> render frame input
  -> renderer
```

The shared contract should stay small and concrete. It should mostly wrap the
existing CPU-side draw inputs rather than inventing a broad render IR:

- sprites,
- edge fans,
- colored overlays,
- textured overlays,
- static and dynamic layers,
- explicit draw phase / sprite role metadata,
- optional per-frame timing values needed by animated shaders.

Avoid adding `MaterialId`, a generic material graph, a full frame graph, or a
trait-heavy pass system until concrete feature pressure requires it.

## Non-Goals

- Do not introduce ECS, plugin architecture, scripting, or a reusable external
  engine crate.
- Do not abstract over multiple renderer backends; `wgpu` is the backend.
- Do not build automatic render-target aliasing, barrier scheduling, or a full
  frame graph. The current fixed pass order is a good fit.
- Do not move all fixture assembly out of `commands/` unless an extracted
  helper has an actual second caller or removes meaningful duplication.
- Do not relax RimWorld parity requirements or replace visual proof with unit
  tests alone.

## Design Principles

- Move mechanically before redesigning. First split the fat renderer module so
  the real seams are visible in smaller files.
- Keep public behavior stable after each slice. This refactor should be
  shippable in small commits.
- Express renderer policy with named data, not loose booleans.
- Keep app concepts app-owned. A fixture, pawn, terrain def, or RimWorld
  section-layer concept should not need to know about backend internals.
- Keep renderer concepts renderer-owned. Pipelines, bind group layouts,
  offscreen targets, readback buffers, and `wgpu` resource lifetimes should not
  leak into commands or runtime.
- Use fixture renders as acceptance evidence whenever pass ordering, water,
  fog, snow, lighting, shadows, or dynamic sprite ordering might change.

## Proposed Module Direction

The exact names can change during implementation, but the intended ownership is:

- `renderer/mod.rs`: public renderer facade and module exports.
- `renderer/context.rs`: surface, adapter, device, queue, config, resize.
- `renderer/camera.rs`: camera state, uniforms, screen-to-world conversion.
- `renderer/pipelines.rs`: shader modules, layouts, and render pipelines.
- `renderer/textures.rs`: texture upload, texture deduplication, bind groups.
- `renderer/batches.rs`: sprite, edge, colored overlay, and textured overlay
  batching.
- `renderer/passes.rs`: ordered pass execution, including water depth and
  surface draws.
- `renderer/screenshot.rs`: readback and PNG write-if-changed behavior.
- `render_frame.rs` or `engine/render_frame.rs`: neutral CPU-side frame input
  types shared by static fixture rendering and live runtime rendering.

Keep this as a module split first. Promote to a workspace crate only if
enforcing the boundary becomes valuable.

## Ordered Slices

## Slice 1: Mechanical Renderer Module Split

Move `src/renderer.rs` into a `src/renderer/` module tree without intentionally
changing behavior or public contracts.

Suggested extraction order:

1. Public input/output types and constants.
2. Camera state and camera uniform updates.
3. Texture upload/deduplication helpers.
4. Batch packing and validation helpers.
5. Pipeline and bind group layout creation.
6. Screenshot readback/write helpers.
7. Pass execution helpers.

Acceptance:

- Existing commands compile with minimal import churn.
- Existing unit tests pass.
- `cargo run -- render-fixtures` produces the same fixture render set.
- The diff is mostly moves, not semantic redesign.

## Slice 2: Neutral Render Frame Types

Introduce a small shared frame input type that gathers the existing draw inputs
without creating a broad intermediate representation.

The first version can be close to:

```rust
pub struct RenderFrame {
    pub static_sprites: Vec<SpriteInput>,
    pub dynamic_sprites: Vec<SpriteInstance>,
    pub edge_sprites: Vec<EdgeSpriteInput>,
    pub colored_overlays: Vec<ColoredMeshInput>,
    pub textured_overlays: Vec<TexturedMeshInput>,
}
```

Adjust exact fields based on what the mechanical split reveals. The important
constraint is that both static fixture rendering and live runtime rendering can
produce the same shape.

Acceptance:

- `viewer.rs` consumes a frame-shaped object rather than a loose collection of
  parallel launch vectors where practical.
- Static fixture output and live runtime output can be described with the same
  type family.
- No new generic material abstraction is introduced.

## Slice 3: Replace Renderer Policy Leaks

Move pass/layer intent out of `renderer.rs` and replace boolean sprite routing
with explicit metadata.

Concrete targets:

- Replace `OverlayPass` imports from overlay builders with a neutral
  `DrawPhase`, `FramePhase`, or similarly named app/engine-level enum.
- Replace `is_water` and `is_terrain` with a named role enum such as
  `SpriteRole`.
- Keep the role enum narrow. It only needs to cover known routing needs:
  terrain, ordinary static sprite, dynamic sprite, and water-routed terrain or
  water-routed sprite.

Acceptance:

- Overlay builders no longer import renderer internals to specify composition
  order.
- Sprite routing is readable at call sites and cannot express invalid boolean
  combinations.
- Water still renders through its offscreen depth and surface passes.

## Slice 4: Static Fixture Frame Assembly Boundary

Extract only the frame assembly parts of `fixture_cmd.rs` that are needed to
serve the static fixture entry point cleanly.

The command should still own CLI concerns:

- parse command options,
- load the named fixture,
- choose screenshot/window behavior,
- launch or run headless mode,
- emit command-level diagnostics.

The extracted assembly should own renderer-facing construction:

- terrain sprite assembly,
- thing sprite assembly,
- pawn visual profile and initial pawn sprite assembly,
- terrain edge sprite assembly,
- static overlay assembly,
- required render assets such as noise and water resources.

Acceptance:

- `fixture_cmd.rs` becomes orchestration rather than the place where all draw
  records are built.
- The extracted assembly code has one clear caller initially, but is shaped so
  the live entry point can share pieces when needed.
- No speculative app layer is created beyond the assembly boundary needed by
  the static fixture entry point.

## Slice 5: Live Runtime Frame Assembly Boundary

Update `runtime/v2/render_bridge.rs` so live runtime state produces the same
frame/draw shape as the static fixture path.

Concrete targets:

- Runtime-derived pawn nodes keep using `compose_pawn`.
- Hover, selection, selected-pawn marker, and path markers remain deterministic.
- Texture lookup for dynamic pawn nodes stays explicit and cached.
- Per-frame animation/effect data has an obvious place to flow into the
  renderer without commands knowing about it.

Acceptance:

- `viewer.rs` stays a host: event loop, window events, runtime ticking, and
  submitting frame input to the renderer.
- Runtime frame assembly lives in `runtime/v2/render_bridge.rs` or a similarly
  focused module.
- Live movement and selection visuals remain unchanged in `fixtures/v2/move_lane.ron`
  and `fixtures/v2/obstacle_pathing.ron`.

## Slice 6: Explicit Pass Queue Inside Renderer

Once frame input roles/phases are explicit, make renderer pass ordering explicit
inside the renderer.

Prefer a concrete enum or fixed function sequence over `Vec<Box<dyn Pass>>`
unless the borrow/lifetime shape stays simple and there is clear duplication to
remove.

The pass model should cover:

- water depth offscreen pass,
- before-world overlays,
- terrain sprites and terrain edges,
- after-terrain overlays,
- static sprites,
- after-static overlays,
- dynamic sprites,
- after-dynamic overlays,
- screenshot readback.

Acceptance:

- Adding a new fixed pass has an obvious file and function boundary.
- Water no longer feels bolted into unrelated render setup/draw code.
- Pass order remains easy to compare against RimWorld section-layer notes.

## Validation

At minimum after each semantic slice:

- `cargo +nightly-2025-12-09 fmt --all`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo run -- render-fixtures`

For purely mechanical move-only commits, use judgment, but do not merge a slice
without a full check and fixture render pass. Visual parity is the primary risk
for this refactor.

## Open Questions

- Should `RenderFrame` live under `renderer/` as public input, or under a
  neutral `engine/` or `render_frame` module? Start with the location that
  minimizes churn; move only if imports show the boundary is wrong.
- Should the live entry point reuse static fixture assembly for initial terrain,
  things, and pawn visuals, or should both call smaller shared helpers? Choose
  the smaller extraction when implementation reaches Slice 4.
- Should water be a sprite role, a material kind, or a dedicated terrain shader
  route? Keep the first pass close to current behavior and rename only once the
  call sites make the natural term obvious.
- Which current active visual plan should land first? Avoid interleaving this
  refactor with fog/snow follow-up work unless one directly blocks the other.
