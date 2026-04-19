# Terrain Transitions + Wall Linking — Implementation Plan

Mirror RimWorld's `LinkDrawer` and `TerrainEdge` systems so buildings (walls, sandbags, conduits) link to neighbors and terrain tiles fade into each other by precedence. Phased, commit-granular plan.

---

## 1. Goals

- Walls, sandbags, power conduits, and rock auto-connect to matching neighbors using RimWorld's 4-bit NESW bitmask into a 4×4=16 subimage atlas.
- `CornerFiller` wall type draws extra corner quads to close L/T-junction gaps.
- `MapEdge` flag makes rock and walls link to the out-of-bounds boundary, so rock hugs the map border.
- Terrain tiles with higher `renderPrecedence` draw fading overlays onto lower-precedence neighbors. One-directional: water → soil, never soil → water.
- Three terrain edge styles: `Hard` (sharp pixel cut), `FadeRough` (noise-masked fade), `Water` (noise fade + later waves).
- Pathing: walls block movement, consistent with `ThingSpawn.blocks_movement` already in the fixture schema.

## 2. Non-Goals (Explicitly Deferred)

| Deferred | Why |
|---|---|
| `LinkDrawerType::Transmitter` / `TransmitterOverlay` | Needs a power-net graph we don't have yet |
| `LinkDrawerType::Asymmetric` (fences) | One-off second flag-set; add with fence assets later |
| `Graphic_Appearances` stuff variants (`Smooth` / `Bricks` / `Planks`) | No stuff system yet; default to `Bricks` for walls, `Rock_Atlas` for rock |
| Animated water wave distortion | Pair with shader-distortion work noted in the roadmap |
| Door-linking via `asymmetricLink.linkToDoors` | Doors aren't drawn yet |
| RimWorld's 9-vertex edge mesh + section batching | Overlay-quad approach gets the same visual; the perf upgrade is a contained rewrite at ~250×250 map scale |
| `Custom1..10` link flags | Unused in Core |

## 3. Research Summary (Spec Reference)

### 3.1 `LinkDrawerType` (`Verse.LinkDrawerType` byte enum)

| Value | Graphic class | Behavior |
|---|---|---|
| `None` | `null` | No linking, static graphic |
| `Basic` | `Graphic_Linked` | 16-subimage atlas; NESW bitmask picks one subimage |
| `CornerFiller` | `Graphic_LinkedCornerFiller` | As `Basic`, plus small corner-fill quads at NE/SE/SW/NW when both orthogonal neighbors link AND diagonal cell links |
| `Transmitter` | `Graphic_LinkedTransmitter` | (deferred) |
| `TransmitterOverlay` | `Graphic_LinkedTransmitterOverlay` | (deferred) |

### 3.2 `LinkFlags` (`[Flags]` enum)

```
None         = 0
MapEdge      = 0x01    // sentinel: out-of-bounds counts as match if this is set
Rock         = 0x02
Wall         = 0x04
Sandbags     = 0x08
PowerConduit = 0x10
Barricades   = 0x20
Fences       = 0x40    // inferred (not in older decompiles; next pow-of-2)
```

**Matching rule:** neighbor links iff `neighbor.linkFlags & self.linkFlags != 0`. Same flag set is used for "what I am" and "what I match" (with the `MapEdge` trick for boundary handling).

### 3.3 Atlas layout (all `Graphic_Linked*`)

4×4 grid (16 subimages, whole texture). Per subimage:
- Scale: `0.1875 × 0.1875` UV (= 3/16), with a `0.03125` margin inside each `0.25` quarter.
- UV origin of cell `i`: `x = (i % 4) * 0.25 + 0.03125`, `y = (i / 4) * 0.25 + 0.03125`.
- Unity UV origin is bottom-left. Index 0 = bottom-left visually; 15 = top-right. Note: Unity Y is bottom-up; our shader sampler and image layout need to agree.

Bitmask (NESW ordering from `GenAdj.CardinalDirections`):
```
index = (N ? 1 : 0) + (E ? 2 : 0) + (S ? 4 : 0) + (W ? 8 : 0)
```

Complete table:
| idx | W S E N | shape |
|---|---|---|
| 0  | 0000 | isolated dot |
| 1  | 0001 | stub N |
| 2  | 0010 | stub E |
| 3  | 0011 | L NE |
| 4  | 0100 | stub S |
| 5  | 0101 | vertical bar |
| 6  | 0110 | L SE |
| 7  | 0111 | T open-W |
| 8  | 1000 | stub W |
| 9  | 1001 | L NW |
| 10 | 1010 | horizontal bar |
| 11 | 1011 | T open-S |
| 12 | 1100 | L SW |
| 13 | 1101 | T open-E |
| 14 | 1110 | T open-N |
| 15 | 1111 | cross |

### 3.4 CornerFiller extras

For each of four diagonal positions (NE, SE, SW, NW): emit a small corner-fill quad iff all three of { both orthogonal neighbors link AND the diagonal cell itself links }. The quad samples UV ≈ `(0.5, 0.6)` of the same atlas — lands on the solid wall-body region. Quad is a quarter of a cell, positioned in the appropriate cell corner. At map edges, corner quads are scaled 5× to cover the void (deferred — only relevant with `MapEdge` sentinel combined with corner math).

### 3.5 Terrain edge system

`TerrainDef.renderPrecedence: i32` (default 0). Higher = drawn over lower. For each cell and each of 8 neighbors:
- Iff `neighbor.edgeType != None` AND `neighbor.renderPrecedence >= self.renderPrecedence`, draw the neighbor's terrain as an edge overlay on THIS cell, fading from the shared border inward.
- Equal precedence → neither draws (no infinite recursion).

`TerrainEdgeType`:
- `None`: no edge drawn
- `Hard`: sharp pixel cut, no fade (concrete, floors)
- `FadeRough`: alpha fade × noise mask (`RoughAlphaAdd` / `_AlphaAddTex`)
- `Water`: animated fade + waves (we implement static for now, save waves for later shader work)

Core precedence values (for fixtures):
```
  0  Underwall           Hard
 70  Concrete            Hard
340  Soil                FadeRough
350  Sand, Ice           FadeRough
394  WaterShallow        Water
395  WaterDeep           Water
400  Bridge              Hard
```

### 3.6 Concrete XML references

```xml
<!-- Wall (walls link to walls and rock) -->
<graphicData>
  <texPath>Things/Building/Linked/Wall</texPath>
  <graphicClass>Graphic_Appearances</graphicClass>
  <linkType>CornerFiller</linkType>
  <linkFlags><li>Wall</li><li>Rock</li></linkFlags>
</graphicData>

<!-- Rock (rock + map edges) -->
<linkFlags><li>Rock</li><li>MapEdge</li></linkFlags>
<linkType>CornerFiller</linkType>

<!-- Sandbags -->
<linkType>Basic</linkType>
<linkFlags><li>Sandbags</li></linkFlags>

<!-- PowerConduit (deferred) -->
<linkType>Transmitter</linkType>
<linkFlags><li>PowerConduit</li></linkFlags>
```

### 3.7 Assets already extracted (confirmed in `target/investigation/extract_packed_with_tpk/`)

- `Wall_Atlas_Bricks_439.png` — wall atlas, 320×320, Bricks variant
- `RoughAlphaAdd_3230.png` — noise mask for FadeRough edges (`_AlphaAddTex`)
- `Edge_1741.png` — terrain edge helper sprite

Paths in the game: `Things/Building/Linked/Wall_Atlas_Bricks`, `Things/Building/Linked/Rock_Atlas`, `Things/Building/Linked/Sandbags_Atlas`, and the shared `RoughAlphaAdd`.

## 4. Current State (Codebase)

| Concern | Current file:line | State |
|---|---|---|
| Terrain rendering | `commands/fixture_cmd.rs:118-153` | 1 quad per cell, depth `-1.0`, no edges |
| Thing rendering | `commands/fixture_cmd.rs:156-201` | 1 quad per thing, depth `~-0.8`, no linking |
| `TerrainDef` | `defs.rs:43-47` | Has `edge_texture_path` parsed, unused; no `edgeType`, no `renderPrecedence` |
| `ThingDef.GraphicData` | `defs.rs:28-34` | Has `graphic_class` parsed, unused; no `linkType`, no `linkFlags` |
| `WorldState` | `world/state.rs:66-90` | Flat `Vec<TerrainTile>` + `Vec<ThingState>`; no neighbor queries, no cell-index helpers |
| `PathGrid` | `path/grid.rs` | Built by `tick::build_path_grid(world)`, discarded after use; not stored on world |
| Renderer `InstanceData` | `renderer.rs:90-134` | `world_pos[3] + size[2] + tint[4]` = 48 B with pads; no UV sub-rect |
| Shader | `shader.wgsl` | Single pipeline, samples one `sprite_tex`; no noise binding |
| Fixture schema | `fixtures/schema.rs:14-33` | `ThingSpawn.blocks_movement` exists; no rotation, no link flags (that's fine — they come from defs) |

No grep hits in `src/` for `link`, `autotile`, `transition`, `LinkDrawer`, or `LinkFlags`. Greenfield.

## 5. Architecture Decisions

### 5.1 UV sub-rect on `InstanceData`

Add `uv_rect: [f32; 4]` = `(u_min, v_min, u_max, v_max)`. Default constructor passes `(0, 0, 1, 1)` for backward compat. The vertex shader interpolates UV within the instance's sub-rect instead of the fixed-quad `uv` attribute. Layout ends at 64 B (16-aligned); reshuffle the existing `_pad0` / `_pad1` bytes.

```
struct InstanceData {
    world_pos: [f32; 3], _pad0: f32,     // 16
    size:       [f32; 2], _pad1: [f32;2], // 16
    tint:       [f32; 4],                 // 16
    uv_rect:    [f32; 4],                 // 16
}                                         // = 64
```

WGSL: new `@location(5) uv_rect: vec4<f32>` in `VsIn`. Vertex shader: `out.uv = mix(uv_rect.xy, uv_rect.zw, vec2(uv_attr.x, 1.0 - uv_attr.y))` (or equivalent, matching the existing quad's UV orientation).

### 5.2 Second pipeline for terrain edges

Terrain edges need a different fragment shader (noise mask × directional fade). Options considered:

- **Shader-branch (single pipeline):** flag on `InstanceData`, if-branch in fragment. Rejected — noise mask must always be bound, adds complexity to every sprite.
- **Separate pipeline (chosen):** new `edge_pipeline` with its own shader, binds `_AlphaAddTex` as a second texture. Renderer keeps a sprite-kind tag on each batch and switches pipelines in `render()`. Base pipeline is unchanged.

New bind group layout for the edge pipeline:
- `@group(1) @binding(0)` = sprite_tex (neighbor terrain) — same as base
- `@group(1) @binding(1)` = sprite_sampler — same as base
- `@group(2) @binding(0)` = noise_tex (shared `RoughAlphaAdd`)
- `@group(2) @binding(1)` = noise_sampler (wrap-repeat)

Group 2 is bound once per frame on the edge pipeline.

### 5.3 Edge sprite encoding

For each (cell, direction) where an overlay must be drawn, emit one `RenderSprite` with:
- `texture_id` = neighbor terrain texture
- `world_pos` = this cell center, `z = -0.95` (between terrain base -1.0 and things -0.8)
- `size` = 1×1
- `uv_rect` = `(0, 0, 1, 1)` (sample neighbor terrain full-texture)
- `tint.a` = one of `EDGE_FADE_N/E/S/W` sentinels encoded as negative values that the shader decodes

Rather than overload `tint`, cleaner: add `edge_mask: [f32; 4]` where each channel = strength of fade from that edge (1.0 = full bleed from N, 0.0 = none). This lives in a separate instance buffer only for the edge pipeline — the base pipeline's `InstanceData` doesn't need it.

Decision: the edge pipeline has its own `EdgeInstanceData` struct, disjoint from `InstanceData`. It shares the quad geometry (vertex buffer) but has its own attribute layout. Keeps base pipeline lean.

```
struct EdgeInstanceData {
    world_pos: [f32; 3], _pad0: f32,     // 16
    size:       [f32; 2], _pad1: [f32;2], // 16
    edge_mask:  [f32; 4],                 // 16   // N, E, S, W strengths (0..1)
    tint:       [f32; 4],                 // 16
    noise_seed: [f32; 2], _pad2: [f32;2], // 16   // world-space seed so adjacent cells don't tile the noise identically
}                                         // = 80
```

The edge fragment shader computes:
```
local = vec2(uv.x, 1.0 - uv.y)               // 0..1 inside cell
alpha_dir = max(
    mask.x * smoothstep(0.0, fade_width, 1 - local.y),  // N bleed fades out going south
    mask.y * smoothstep(0.0, fade_width, 1 - local.x),
    mask.z * smoothstep(0.0, fade_width, local.y),
    mask.w * smoothstep(0.0, fade_width, local.x),
)
noise = textureSample(noise_tex, noise_sampler, local * NOISE_SCALE + noise_seed).r
alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0)   // FadeRough
// Hard: alpha = alpha_dir > 0.5 ? 1.0 : 0.0
// Water: alpha = FadeRough, later + wave distort
final = textureSample(sprite_tex, sprite_sampler, uv) * tint
final.a *= alpha
```

Edge type per sprite is encoded via `fade_width` / noise multiplier in a per-instance uniform — but simplest: separate edge_pipeline for each edge type (Hard / FadeRough / Water), chosen at batch build time. Three pipelines total on the edge side. Share the same bind group layout; just different shader entry points.

Decision revised: **one edge pipeline**, with `edge_type: u32` packed into `_pad2`. Shader branches on it. Keeps pipeline count at 2 (base + edge).

### 5.4 Neighbor queries on `WorldState`

Add to `WorldState`:
```rust
pub fn cell_in_bounds(&self, cell: Cell) -> bool;
pub fn cell_index(&self, cell: Cell) -> Option<usize>;
pub fn terrain_at(&self, cell: Cell) -> Option<&TerrainTile>;
pub fn things_at(&self, cell: Cell) -> &[usize];   // returns thing-indices
pub fn cardinal_neighbors(cell: Cell) -> [Cell; 4]; // N, E, S, W (associated fn)
pub fn diagonal_neighbors(cell: Cell) -> [Cell; 4]; // NE, SE, SW, NW
```

Per-cell thing index: `Vec<Vec<usize>>` built eagerly in `world_from_fixture`, stored as `thing_grid: Vec<Vec<usize>>` on `WorldState`, kept in sync if the world ever mutates things (v2 runtime doesn't move things today, so minimal concern).

Constant depth-layer values as module-level consts:
```rust
pub const DEPTH_TERRAIN_BASE:  f32 = -1.00;
pub const DEPTH_TERRAIN_EDGE:  f32 = -0.95;
pub const DEPTH_FLOOR:         f32 = -0.90;
pub const DEPTH_THING_STATIC:  f32 = -0.80;
pub const DEPTH_WALL:          f32 = -0.70;
pub const DEPTH_WALL_CORNER:   f32 = -0.69;
// Pawns stay ~0.0
```

### 5.5 Where the new types live

- `src/linking.rs` — `LinkFlags` bitset + `LinkDrawerType` enum + `link_index()` + `atlas_uv_rect()` + `corner_filler_positions()` pure functions. No I/O, no renderer deps. Fully unit-testable.
- `src/defs.rs` — extend `GraphicData` with `link_type: LinkDrawerType`, `link_flags: LinkFlags`. Extend `TerrainDef` with `edge_type: TerrainEdgeType`, `render_precedence: i32`. Add XML parsing.
- `src/world/neighbors.rs` (new small module) or extend `src/world/query.rs` — the new accessors.
- `src/render/edge_pipeline.rs` (new) — `EdgeInstanceData`, pipeline creation, bind groups. `Renderer` holds it alongside the base pipeline.
- `src/commands/linking_sprites.rs` (new) — emits the link-drawn and edge sprites from `WorldState` + defs, called from `fixture_cmd.rs::build_world_sprites`.

### 5.6 Atlas asset resolution

Walls default to `Things/Building/Linked/Wall_Atlas_Bricks` for now (no stuff variant). The atlas is resolved once via the existing `resolve_texture_path`, cached by `TextureId`, and all 16 sub-rects reference it.

For `Graphic_Appearances` we note the chosen default with a TODO pointer in code so the stuff system can slot in cleanly later.

## 6. Phase Breakdown

Each phase = 1 commit. Tests run and pass at the end of every phase.

### Phase A — Renderer Infrastructure

**Goal:** UV sub-rect support in the base pipeline; edge pipeline scaffolding; neighbor-query API; depth constants.

**Files changed:**
- `src/renderer.rs` — add `uv_rect` to `InstanceData`, update `desc()`, update `InstanceData::from_params`. Add optional `uv_rect` to `SpriteParams` (default full texture). Add `edge_pipeline` field + creation. Add `edge_sprite_batches` vec + render-pass extension.
- `src/shader.wgsl` — extend `VsIn` / `VsOut` with `uv_rect`, compute final UV in `vs_main`. No fragment change.
- `src/render/mod.rs` (new) + `src/render/edge_pipeline.rs` (new) — `EdgeInstanceData`, pipeline + bind group creation, `EdgeSpriteBatch`. Loaded noise texture binding.
- `src/edge_shader.wgsl` (new) — fragment shader with directional fade × noise × edge_type branch.
- `src/world/state.rs` — add `thing_grid: Vec<Vec<usize>>` field, populated in `world_from_fixture`.
- `src/world/neighbors.rs` (new) — `cell_in_bounds`, `cell_index`, `terrain_at`, `things_at`, `cardinal_neighbors`, `diagonal_neighbors`, depth constants. Re-exported from `src/world/mod.rs`.
- `src/world/spawn.rs` — build `thing_grid` when constructing `WorldState`.
- `src/main.rs` / callers — if any touch `SpriteParams` directly, add `.with_uv_rect(...)` or accept defaulted.

**Key changes in detail:**

`SpriteParams`:
```rust
#[derive(Debug, Clone)]
pub struct SpriteParams {
    pub world_pos: Vec3,
    pub size: Vec2,
    pub tint: [f32; 4],
    pub uv_rect: [f32; 4],   // default (0,0,1,1)
}
```

Add `SpriteParams::with_uv_rect(self, [u0,v0,u1,v1]) -> Self` convenience.

`shader.wgsl` vertex logic:
```wgsl
out.uv = vec2<f32>(
    mix(uv_rect.x, uv_rect.z, uv_attr.x),
    mix(uv_rect.y, uv_rect.w, uv_attr.y)
);
```

Renderer: separate `sprite_batches: Vec<SpriteBatch>` (existing) and `edge_sprite_batches: Vec<EdgeSpriteBatch>`. `rebuild_sprite_batches` splits into both based on a new `kind: SpriteKind` tag on `SpriteInput`/`SpriteInstance`:

```rust
pub enum SpriteKind { Base, Edge(EdgeInstanceExtras) }
pub struct EdgeInstanceExtras {
    pub edge_mask: [f32; 4],
    pub edge_type: EdgeType,
    pub noise_seed: [f32; 2],
}
```

Render pass draws base batches first (sorted by `min_z` as today), then edge batches interleaved by z — actually simpler: concatenate, sort by min_z, each batch carries a pointer to its pipeline. But the bind group layout differs, so pass toggles pipeline + group(2) binding only when switching kinds. Implementation: sort all batches globally by min_z; during draw, compare each batch's pipeline to the currently-bound and switch only on change.

Noise texture: load `Things/Misc/RoughAlphaAdd` (or exact packed name — resolve via asset resolver) at renderer init. Store in a dedicated bind group, built once.

**Tests:**
- Unit test for `InstanceData` size/alignment (bytemuck round-trip) to lock the layout.
- `cargo test` on new neighbor helpers: in/out of bounds, wrap, cell_index.
- Smoke: run an existing fixture, confirm screenshot matches pre-phase baseline (no visual change since all callers default to full UV).

**Done criteria:** existing fixtures render identically; `cargo build && cargo test && cargo clippy` clean.

---

### Phase B — Def Plumbing (XML parsing + types)

**Goal:** Parse `linkType`, `linkFlags`, `renderPrecedence`, `edgeType` from XML. Add the enum/bitset types. Consume `graphic_class` to validate or select linking behavior.

**Files changed:**
- `src/linking.rs` (new) — `LinkFlags` (bitflags), `LinkDrawerType` enum, `TerrainEdgeType` enum. XML string → enum parsing. `link_index(self_flags, neighbor_flags_array: [Option<LinkFlags>; 4]) -> u8`. `atlas_uv_rect(index: u8) -> [f32; 4]`. `corner_filler_positions(bitmask: u8, diagonal_has_link: [bool; 4]) -> SmallVec<[DiagonalCorner; 4]>`.
- `src/defs.rs` — extend `GraphicData`:
  ```rust
  pub struct GraphicData {
      pub tex_path: String,
      pub graphic_class: Option<String>,
      pub color: RgbaColor,
      pub draw_size: Vec2,
      pub draw_offset: Vec3,
      pub link_type: LinkDrawerType,     // default None
      pub link_flags: LinkFlags,         // default empty
  }
  ```
  Extend `TerrainDef`:
  ```rust
  pub struct TerrainDef {
      pub def_name: String,
      pub texture_path: String,
      pub edge_texture_path: Option<String>,
      pub edge_type: TerrainEdgeType,    // default None
      pub render_precedence: i32,        // default 0
  }
  ```
- `parse_graphic_data` reads `<linkType>` child, maps string → enum. Reads `<linkFlags><li>…</li></linkFlags>`, folds into bitflags. Unknown tokens log-warn once.
- `parse_terrain_def` reads `<edgeType>` and `<renderPrecedence>`.
- Add light warning if `graphic_class` is `Graphic_Linked*` but `link_type == None` (data inconsistency), and vice versa — log once per def, don't fail.

**Tests in `src/linking.rs`** (embedded `#[cfg(test)]`):
- `link_index` across 16 NESW combinations with matched vs mismatched flag masks.
- `link_index` with `MapEdge`: out-of-bounds neighbors modeled by passing `None`; if self has `MapEdge`, `None` counts as link.
- `atlas_uv_rect` returns correct 0.1875×0.1875 rect with 0.03125 margin for all 16 indices; verifies UV origin convention (index 0 = bottom-left).
- `LinkFlags` parse from strings: "Wall", "Rock", "MapEdge", "Sandbags", "PowerConduit", "Barricades", "Fences". Case-sensitive, matches RimWorld spelling.
- `LinkDrawerType::from_str` for "None", "Basic", "CornerFiller"; `Transmitter`/`TransmitterOverlay` parsed but marked as unsupported at use sites.
- `TerrainEdgeType::from_str` for "None", "Hard", "FadeRough", "Water".

**Tests in `src/defs.rs`:**
- Inline XML fixture strings parsed via `parse_graphic_data` and `parse_terrain_def`, asserting enum + bitmask + precedence extracted correctly. Sample inputs include Wall, Rock, Sandbags, Concrete, WaterShallow.

**Done criteria:** every installed `ThingDef` and `TerrainDef` parses without panic. `cargo test` includes the new parse tests.

---

### Phase C — Wall Linking (Basic + CornerFiller)

**Goal:** Emit wall sprites via atlas sub-rect based on cardinal neighbor matching. Emit corner-filler quads for `CornerFiller` type.

**Files changed:**
- `src/commands/linking_sprites.rs` (new) — `emit_linked_thing_sprite(ctx, thing, thing_def, world) -> Vec<RenderSprite>`. Called from `build_world_sprites` when `thing_def.graphic_data.link_type != None`. Loops:
  1. Compute cardinal neighbor flags:
     ```rust
     for dir in CARDINALS { 
         let neighbor_flags = world.things_at(cell + dir)
             .iter()
             .find_map(|idx| ctx.defs.thing_defs[&world.things()[*idx].def_name].graphic_data.link_flags)
             .or_else(|| if !world.cell_in_bounds(cell + dir) && self_flags.contains(MapEdge) { Some(self_flags | MapEdge) } else { None });
     }
     ```
  2. `let index = link_index(self_flags, neighbor_flags)`
  3. Emit primary sprite with `uv_rect = atlas_uv_rect(index)`, depth `DEPTH_WALL`, size 1×1.
  4. If `CornerFiller`: evaluate each of 4 diagonals. For each diagonal where both orthogonals AND the diagonal cell all link, emit a 0.5×0.5 quad positioned in the corner of THIS cell, UV = a constant "solid body" rect (per research, ≈ centered around (0.5, 0.6) of the atlas — make it a named constant `CORNER_FILL_UV_RECT`). Depth `DEPTH_WALL_CORNER`.

- `src/commands/fixture_cmd.rs::build_world_sprites` — when iterating things, branch:
  ```rust
  if thing_def.graphic_data.link_type != LinkDrawerType::None {
      sprites.extend(emit_linked_thing_sprite(...)?);
  } else {
      // existing single-sprite emission
  }
  ```

- `src/assets/resolver.rs` — extend `resolve_thing` to handle `Graphic_Appearances` / linked atlases. For now: if `link_type != None`, look up the atlas tex path using a default stuff variant (`<texPath>_Atlas_Bricks` for walls, `<texPath>` suffixed or literal for others). Add a helper `resolve_linked_atlas(texPath, link_type) -> SpriteAsset` that applies the naming rule.

**Tests in `src/linking.rs`:**
- Corner filler emission: table of (bitmask, diagonal-presence) → expected corner count & positions. 16 cases × 4 diagonals.

**Fixture under `fixtures/v2/walls_patterns.ron`:**
- 20×10 map of Soil.
- Isolated wall at (1,1).
- Horizontal wall run: (3..8, 1).
- Vertical wall run: (1, 3..6).
- L-corner at (10, 3)-(10, 4)-(11, 4).
- T-junction at (13, 3)-(13, 4)-(13, 5)-(14, 4).
- Plus-shape at (17, 4)-(18, 3)-(18, 4)-(18, 5)-(19, 4).
- Rock along y=0 and y=9 to test `MapEdge` (rock should link to map edge).

**Screenshot test:** `cargo run -- fixture v2 walls_patterns --screenshot walls.png`, check against a committed reference image (ignore small pixel diffs using an existing screenshot-diff tool or a byte-exact comparison to start). If no diff tooling exists, document the expected visual and commit the screenshot under `plans/terrain-walls-linking/reference/`.

**Done criteria:** walls render connected, cornerfiller closes gaps, rock hugs map edge, no regressions in existing fixtures.

---

### Phase D — Terrain Edges (precedence-driven fade overlays)

**Goal:** Higher-precedence terrain fades into lower-precedence neighbors using the edge pipeline + noise mask.

**Files changed:**
- `src/commands/linking_sprites.rs` — add `emit_terrain_edge_sprites(ctx, world) -> Vec<EdgeSpriteInput>`. For each cell, for each of 4 cardinal neighbors:
  ```rust
  let self_def = terrain_defs[&terrain_at(cell)];
  let n_def = terrain_defs[&terrain_at(neighbor)];
  if n_def.edge_type == None { continue; }
  if n_def.render_precedence <= self_def.render_precedence { continue; }
  // emit overlay of neighbor's terrain onto this cell, with edge-mask bit for this direction
  ```
  Accumulate per (cell, neighbor_terrain_id): combine up to 4 direction bits into one sprite with a 4-float mask. Produces at most 1 sprite per unique neighbor terrain per cell (typically 1–2 per edge cell).
- `src/render/edge_pipeline.rs` — `EdgeInstanceData`, `EdgeSpriteBatch`, `EdgeSpriteInput { image, params: EdgeSpriteParams }`. Renderer method `set_static_edge_sprites(Vec<EdgeSpriteInput>) -> Result<()>`.
- `src/commands/fixture_cmd.rs` — after building base static sprites, call `emit_terrain_edge_sprites` and submit via `renderer.set_static_edge_sprites`.
- `src/edge_shader.wgsl` — fragment shader per Section 5.3. `edge_type` uniform-branch selects Hard / FadeRough / Water behavior:
  ```wgsl
  switch edge_type {
    case 0u: { alpha = select(0.0, 1.0, alpha_dir > 0.5); }  // Hard
    case 1u: { alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0); }  // FadeRough
    case 2u: { alpha = clamp(alpha_dir * (0.5 + noise), 0.0, 1.0); }  // Water (same as rough for now)
    default: { alpha = 0.0; }
  }
  ```
- `src/renderer.rs::new` — load noise texture (`Things/Misc/RoughAlphaAdd` via resolver, fallback to bundled bytes if missing). Create noise bind group.
- `src/main.rs` / dispatch — no changes; the emission is internal.

**Noise seed calculation:** `noise_seed = vec2(cell.x as f32 * NOISE_STEP, cell.z as f32 * NOISE_STEP)` where `NOISE_STEP` is a small irrational increment (e.g. `0.31`) to avoid visible tiling across cells.

**Fade width:** start at `0.35` (35% of a cell fades to transparent across the edge). Tunable; expose as a const, not a def field.

**Tests:**
- Unit test for the emission logic: a 3×3 grid with Soil + one WaterShallow in the center emits exactly 4 edge sprites (one per cardinal direction of the center cell's neighbors — wait, actually 4 for the 4 Soil cells around the center, each receiving a water-edge overlay). Confirm count, positions, direction masks.
- Unit test for precedence: equal-precedence neighbors emit zero edges.
- Visual fixture `fixtures/v2/terrain_mix.ron`: a 20×20 map with pockets of Soil, Sand, WaterShallow, WaterDeep, Concrete. Screenshot-compared.

**Done criteria:** water fades into soil, soil stays put against concrete (higher concrete draws Hard over soil), equal-terrain neighbors have no seam.

---

### Phase E — Integration, Fixtures, Pathgrid

**Goal:** Wire walls into pathing. Update existing fixtures to show off the new features. Polish.

**Files changed:**
- `src/world/tick.rs::build_path_grid` — ensures walls block. Already works via `ThingState.blocks_movement`; just confirm walls come through as `blocks_movement: true` in fixtures. If not, set a default: any thing with `link_flags.contains(Wall | Rock)` blocks by default, overridable by explicit fixture setting.
- `src/world/spawn.rs` — when building `ThingState` from `ThingSpawn`, if fixture didn't specify `blocks_movement`, derive from def's link flags.
- Update `fixtures/v2/mixed_things_pawns.ron` — add a wall around part of the scene to exercise pathing with walls.
- Add `fixtures/v2/walls_patterns.ron` (from Phase C) and `fixtures/v2/terrain_mix.ron` (from Phase D) to the fixture index.
- Documentation: a short `plans/terrain-walls-linking/README.md` summarizing which fixtures demo which feature.

**Tests:**
- `tests/v2_fixture_smoke.rs` — add cases for new fixtures.
- Pathing integration: a pawn path command around a wall obstacle in `mixed_things_pawns.ron`.

**Done criteria:** all three showcase fixtures run; pawns respect walls; screenshots match references.

---

### Phase F — Testing, Polish, Snapshot

**Goal:** Harden with property tests, lint clean, commit screenshots.

**Actions:**
- Property tests in `src/linking.rs`: `link_index` is symmetric in a specific sense — if A links to B's direction, B links to A's opposite direction. Sanity-check with `proptest` if it's already a dev-dep, else hand-written.
- Ensure `cargo clippy -- -D warnings` passes across new modules.
- Commit reference screenshots under `plans/terrain-walls-linking/reference/`:
  - `walls_patterns.png`
  - `terrain_mix.png`
  - `mixed_things_pawns.png` (updated)
- Update `docs/vision.md`? No — doesn't mention linking. Skip.
- If any new TODOs are non-obvious (e.g. stuff-variant handling, Transmitter), note in `plans/terrain-walls-linking/followups.md`.

**Done criteria:** clippy clean, all tests green, reference screenshots committed, followups noted.

---

## 7. Commit Plan

Single branch, 6 commits. Each compiles, tests pass, clippy clean.

1. **Phase A** — `add UV sub-rect to sprite pipeline and edge pipeline scaffold`
2. **Phase B** — `parse linkType, linkFlags, edgeType, renderPrecedence from XML`
3. **Phase C** — `wall linking with Basic + CornerFiller atlas selection`
4. **Phase D** — `terrain edge overlays with precedence and noise fade`
5. **Phase E** — `wire walls into pathgrid and add showcase fixtures`
6. **Phase F** — `reference screenshots and followup notes`

If Phase A's renderer surgery gets large, split into A1 (UV rect on base pipeline) + A2 (edge pipeline scaffold + noise binding). Judgment call at the time.

## 8. Risks + Open Questions

### 8.1 Known risks

- **UV orientation mismatch.** Unity atlases are bottom-up; the current quad geometry has `uv = [0,1]` at bottom-left. Easy to get the atlas sub-rect flipped vertically. Mitigation: a test that renders a known subimage (e.g. cross shape at index 15) and byte-compares the center pixel against the known atlas pixel.
- **`RoughAlphaAdd` asset resolution.** The investigation extract has the file, but its in-game path may not resolve via the normal `Things/...` resolver. May need to add a special path or a bundled fallback. Worst case: include the PNG in `assets/` as a baked-in resource.
- **`CornerFiller` UV sample `(0.5, 0.6)`.** Research says the fill quad samples a "solid body" region of the atlas — but the exact location depends on the atlas. For Wall_Atlas_Bricks it should be a solid brick region; for Sandbags it's a sandbag-center. If (0.5, 0.6) looks wrong for one atlas, we may need a per-thing-def fill-UV override. Keep the constant named and commented, adjust as-needed.
- **Performance ceiling.** Per-cell edge sprites at 250×250 = up to 250k overlay instances. At our current fixture scale (≤20×20) this is irrelevant. The upgrade path (section batching + 9-vertex mesh) is documented and contained, but it's the first future-work item when map size grows.
- **`thing_grid` staleness.** If v2 runtime ever moves things (it doesn't today), the grid becomes stale. Add a debug-assert in `cardinal_neighbors`-based wall lookup that the grid is consistent with `world.things()` in debug builds.

### 8.2 Decisions to confirm with Joe during execution

- **Default wall stuff variant.** Plan proposes `Wall_Atlas_Bricks` as the default. Acceptable, or prefer `Smooth` / `Planks`?
- **Snapshot testing infrastructure.** Is there an existing screenshot-diff mechanism I should plug into, or do I commit reference images and visually inspect? Current plan: commit reference images, no automated diff.
- **Hard edge implementation.** Proposed binary-threshold alpha inside the edge shader. Acceptable, or prefer a separate non-blended draw without the noise texture bound?

### 8.3 Outside scope but noted for future

- Stuff-variant appearances (Smooth / Bricks / Planks) require a stuff system — when walls carry a stuff param, `resolve_linked_atlas` needs to pick the correct atlas.
- Transmitter / TransmitterOverlay needs a power-net graph.
- Asymmetric (fences) uses a second flag set; requires extending `GraphicData` with an `asymmetric_link_flags` field.
- 9-vertex edge mesh + section batching for scaling past ~100 cells per side.
- Water wave distortion — add alongside the shader-distortion roadmap item.
- Animated terrain (e.g. water flow) — per-frame uniform (`time`) → shader needs a time uniform, which doesn't exist yet.

## 9. Definition of Done (Whole Feature)

- Walls, sandbags, rock, power-conduit-stub (renders as Basic even without power net) all link correctly on the cardinal bitmask.
- `CornerFiller` walls fill diagonal gaps.
- Rock hugs map edges via `MapEdge`.
- Water / sand / soil / concrete fade or cut correctly per `renderPrecedence` + `edgeType`.
- Three reference fixtures (`walls_patterns`, `terrain_mix`, `mixed_things_pawns` updated) render clean and commit reference screenshots.
- `cargo build && cargo test && cargo clippy -- -D warnings` all pass.
- No regressions in existing fixtures (visual check).
- Plan doc + followups doc committed.
