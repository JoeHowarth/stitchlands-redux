# Plan — Terrain Edges: 9-vertex fan emission

## 1. Why

Our current terrain-edge overlay (`TerrainEdgeContribution::edge_mask: [f32;4]`,
N/E/S/W) emits a single 4-vertex quad per cell-per-neighbor-def. Per-side
alpha is computed in the fragment shader from local quad coords. This is
cardinal-only: diagonal neighbors are ignored entirely, and corner alpha
is a bilinear interpolation of two side values across a flat quad.

Visible in `plans/terrain-walls-linking/reference/terrain_mix.png`: the Ice
pocket has blocky convex corners (outside-of-corner Soil cell has Ice only on
a diagonal, so it never fades) and pinched concave corners (inside-of-corner
Soil cell has two cardinal Ice sides, but the 4-vert quad can't round the
shared corner smoothly).

RimWorld solves this in `Verse/SectionLayer_Terrain.cs:71-138`: inspect all
**8** surrounding cells, and per overlaid neighbor-def emit a **9-vertex fan**
(8 perimeter points + 1 center) with per-vertex alpha. The triangulation +
interpolation produces a clean rounded fade at both convex and concave
corners.

## 2. Reference — RimWorld's emission rule

Source: `Verse/SectionLayer_Terrain.cs:91-139`.

For each overlay neighbor-def `item2` on a cell:

```
array2[0..8] = all false
for k in 0..8:
    if array[k] == item2:
        if k % 2 == 0:                        // cardinal
            array2[(k-1+8)%8] = true
            array2[k]         = true
            array2[(k+1)%8]   = true
        else:                                  // diagonal
            array2[k]         = true
```

`array2[0..8]` are vertex alphas on the **perimeter** (ordered S mid, SW,
W mid, NW, N mid, NE, E mid, SE); vertex 8 (center) is always clear.
Triangulation: 8 fan triangles `(m, (m+1)%8, 8)` for m in 0..8.

Perimeter vertex positions in cell-local coords (cell spans [0,1]²):

```
0 S mid   (0.5, 0)
1 SW      (0,   0)
2 W mid   (0,   0.5)
3 NW      (0,   1)
4 N mid   (0.5, 1)
5 NE      (1,   1)
6 E mid   (1,   0.5)
7 SE      (1,   0)
8 center  (0.5, 0.5)
```

Neighbor array order (`GenAdj.AdjacentCellsAroundBottom`, per
`SectionLayer_Terrain.cs:71-90`): **S, SW, W, NW, N, NE, E, SE**.

Precedence test (`SectionLayer_Terrain.cs:86`): `neighbor.renderPrecedence
>= self.renderPrecedence` and `neighbor.edgeType != Hard` and neither cell
has a foundation. Note `>=` not `>`; equal-precedence neighbors still emit
(so two FadeRough terrains with the same precedence each overlay onto the
other — they cross-fade). We currently use strict `>`; the plan updates this.

## 3. Target

A plan for extending our edge pipeline to match RimWorld's 8-neighbor
9-vertex fan model. Scope is local: the emission helper, the
`EdgeSpriteInput` data shape, the edge render pipeline, and the edge
shader.

Out of scope: water two-pass rendering (separate followup), section
batching, animated waves.

## 4. Files that change

- `src/linking.rs` — new pure helper for the alpha-computation rule, plus
  a `NEIGHBOR_8_ORDER` constant.
- `src/world/neighbors.rs` — add `NEIGHBOR_8_OFFSETS` in RimWorld order;
  keep existing `CARDINAL_OFFSETS` as-is for wall linking.
- `src/commands/linking_sprites.rs` — extend
  `compute_terrain_edge_contributions` to 8 neighbors, replace
  `edge_mask: [f32;4]` with `perimeter_alphas: [f32;8]` on
  `TerrainEdgeContribution`, adjust tests.
- `src/renderer.rs` — replace instanced unit-quad edge pipeline with a
  non-instanced per-vertex fan pipeline. New `EdgeVertex` struct,
  new index buffer (or per-fan indices baked into the vertex list).
- `src/edge_shader.wgsl` — remove `edge_mask` and quad-coord-based fade
  math; use an interpolated vertex alpha instead. Keep noise + edge-type
  branches unchanged.
- `plans/terrain-walls-linking/reference/terrain_mix.png` — regen.

## 5. Phases

### Phase A — Pure helpers and data model

A1. In `src/world/neighbors.rs` add:

```rust
/// 8-neighbor offsets in RimWorld's `AdjacentCellsAroundBottom` order:
/// S, SW, W, NW, N, NE, E, SE.
pub const NEIGHBOR_8_OFFSETS: [(i32, i32); 8] = [
    (0, -1), (-1, -1), (-1, 0), (-1, 1),
    (0,  1), ( 1,  1), ( 1, 0), ( 1, -1),
];
```

Keep the existing `CARDINAL_OFFSETS` untouched — wall linking still uses
the N/E/S/W order.

A2. In `src/linking.rs` add:

```rust
/// Perimeter vertex alphas derived from which of the 8 surrounding cells
/// match the overlay def. Input order matches `NEIGHBOR_8_OFFSETS`
/// (S, SW, W, NW, N, NE, E, SE). Output order matches the perimeter
/// vertex layout (S mid, SW, W mid, NW, N mid, NE, E mid, SE).
/// Mirrors RimWorld's rule at Verse/SectionLayer_Terrain.cs:112-127:
/// cardinal matches set 3 alphas (midpoint + flanking corners);
/// diagonal matches set 1 alpha (the corner).
pub fn perimeter_alphas_from_neighbor_matches(matches: [bool; 8]) -> [f32; 8] { ... }
```

Unit tests to add in `linking.rs`:

- `cardinal_south_match_fills_s_mid_and_flanking_corners` —
  `[true, false, false, false, false, false, false, false]` ⇒ indices
  7, 0, 1 opaque, rest clear.
- `diagonal_nw_match_fills_only_nw_corner` —
  `[false, false, false, true, false, false, false, false]` ⇒ index 3
  opaque, rest clear.
- `full_8_ring_match_fills_all_perimeter` — all eight true ⇒ all eight
  perimeter alphas 1.0.
- `two_adjacent_cardinals_fill_shared_corner_once` — S+W both true, rest
  false ⇒ verts 7, 0, 1, 2, 3 opaque (vertex 1 shared — idempotent).
- `isolated_diagonal_without_cardinals_leaves_cardinal_slot_clear` —
  NE diagonal match only ⇒ vertex 5 opaque; vertex 4 (N mid) and
  vertex 6 (E mid) stay clear. This is the case that fixes the
  convex-corner artifact: Soil diagonally adjacent to Ice gets a
  corner-only overlay.

### Phase B — Emission helper update

B1. Change `TerrainEdgeContribution`:

```rust
pub(crate) struct TerrainEdgeContribution {
    pub cell: Cell,
    pub neighbor_def_name: String,
    pub neighbor_texture_path: String,
    pub perimeter_alphas: [f32; 8],   // was edge_mask: [f32; 4]
    pub edge_type: EdgeType,
}
```

B2. Rewrite `compute_terrain_edge_contributions`:

- Iterate `NEIGHBOR_8_OFFSETS` instead of `CARDINAL_OFFSETS`.
- Precedence test becomes `neighbor.render_precedence >=
  self.render_precedence` (RimWorld rule; was `>`).
- Skip `neighbor.edge_type == None`; Hard stays eligible (RimWorld
  skips Hard at the overlay-emit side — revisit: the C# code tests
  `cellTerrain2.def.edgeType != TerrainEdgeType.Hard`, so we should
  exclude Hard too. Update: add `neighbor.edge_type != Hard` to the
  gate.)
- For each overlay def, build an `[bool; 8]` match array across all 8
  neighbor slots (a def can match multiple slots — merge by OR).
- Call `perimeter_alphas_from_neighbor_matches` once per def to build
  `perimeter_alphas`.

B3. Update the existing four emission tests:

- `higher_precedence_neighbor_emits_onto_lower` — assert
  `perimeter_alphas` opaque at the right verts instead of `edge_mask`.
- `equal_precedence_emits_no_edges` — **flip**: with the new `>=` rule,
  equal-precedence *does* emit. Rename to
  `equal_precedence_cross_emits` and assert both directions contribute.
- `neighbor_with_edge_type_none_skipped` — unchanged.
- `distinct_neighbor_defs_produce_separate_contributions` — unchanged in
  intent, update assertions.

Add one new emission test:

- `convex_corner_produces_corner_only_alpha` — 3×3 fixture with a single
  higher-precedence cell diagonally NE of a base cell (no cardinal
  neighbor matches). Assert the base cell's contribution has only
  `perimeter_alphas[5]` opaque.

### Phase C — Renderer pipeline rewrite

C1. Replace the instanced unit-quad edge pipeline with a non-instanced
per-vertex pipeline.

New vertex struct (`EdgeVertex`, `#[repr(C)] Pod`):

```
world_pos:    [f32; 3]   // absolute world position of this vertex
uv:           [f32; 2]
alpha:        f32
noise_seed:   [f32; 2]
tint:         [f32; 4]
edge_type:    u32        // 0 Hard, 1 FadeRough, 2 Water
_pad:         u32
```

Cell-shared fields (`noise_seed`, `tint`, `edge_type`) are duplicated
across the 9 vertices of one fan. At fixture scale (hundreds of fans)
the duplication is free.

C2. New `EdgeSpriteInput` shape:

```rust
pub struct EdgeSpriteInput {
    pub image: RgbaImage,
    pub fans: Vec<EdgeFan>,
}

pub struct EdgeFan {
    /// 9 vertices in the order [S mid, SW, W mid, NW, N mid, NE, E mid,
    /// SE, center]. Center alpha is always 0.
    pub vertices: [EdgeVertex; 9],
}
```

Batching key stays the same (texture id). Each batched texture gets a
single vertex buffer containing `9 * num_fans` vertices and an index
buffer referencing `8 * num_fans` triangles. Indices per fan:

```
for m in 0..8:
    [base + m, base + (m + 1) % 8, base + 8]
```

Draw call: `pass.set_vertex_buffer(0, verts)` +
`pass.set_index_buffer(indices)` + `pass.draw_indexed(0..n_idx, 0,
0..1)` (no instancing).

C3. Drop `EdgeSpriteParams`, `EdgeInstanceData`, `EdgeSpriteBatch`'s
instance-count field — replace with a per-batch `index_count`. Keep the
texture-bind-group and noise-bind-group hookup unchanged.

C4. Sort order in `rebuild_edge_batches` stays the same (by depth, by
texture id for determinism).

### Phase D — Shader rewrite

`src/edge_shader.wgsl`:

- Drop `edge_mask: vec4<f32>` from `VsIn` / `VsOut`.
- Add `@location(N) alpha: f32` to `VsIn`; pass through to `VsOut`.
- In `fs_main`, remove the four `smoothstep`-per-side calculations.
  Replace `alpha_dir` with the interpolated vertex alpha:

```
let alpha_dir = in.alpha;   // 0..1 from fan interpolation
```

- Keep noise sampling, keep the edge-type switch (Hard threshold,
  FadeRough/Water `alpha_dir * (0.5 + noise)`), keep the base sample.

Rationale: the fade shape now lives in the geometry (alpha-blended fan
triangulation), not in a shader function. This matches RimWorld's
approach — their shader just consumes vertex color alpha.

### Phase E — Fixture verification

E1. `cargo test --bins && cargo clippy`.

E2. Regen reference screenshots:

```
cargo run -- fixture fixtures/v2/terrain_mix.ron \
  --screenshot plans/terrain-walls-linking/reference/terrain_mix.png \
  --no-window
cargo run -- fixture fixtures/v2/mixed_things_pawns.ron \
  --screenshot plans/terrain-walls-linking/reference/mixed_things_pawns.png \
  --no-window
```

Visually verify the Ice pocket in `terrain_mix.png`: convex corners
should round, concave corners should smoothly fill. No regressions to
walls or the Concrete/Soil boundary in `mixed_things_pawns.png`.

### Phase F — Followups update

Remove "RimWorld's 9-vertex edge mesh + section batching" from
`plans/terrain-walls-linking/followups.md` §Non-Goals (the 9-vertex part
is now done; batching can stay listed). Add a short note in the same
file pointing at the new commit.

## 6. Open questions / judgment calls

- **Equal-precedence cross-emit**: tightening to `>=` means two
  equal-precedence FadeRough terrains (e.g. two 350-precedence bodies
  touching) emit onto each other. Each contribution's alpha is driven
  by noise-seeded variation, so visually they produce a fuzzy dithered
  boundary rather than a hard cut. That's RimWorld's behavior. Accept it.
- **Hard-edge gate on overlay**: the C# code gates at
  `cellTerrain2.def.edgeType != Hard`. Hard-edge terrains never
  contribute fades *onto others*, but still get fades drawn *onto them*
  by others. Our current code allows Hard as an overlay source
  (produces a crisp threshold at 0.5 via the shader `case 0u` branch).
  The RimWorld-faithful behavior is to skip. Make it match.
- **Center vertex alpha**: always 0. This pulls the fade *inward from
  the perimeter toward the center* and makes the overlay's interior
  fully transparent, which is correct for "fade neighbor onto self" —
  the neighbor's texture should vanish at the cell center. Sanity
  check: at a fully-surrounded cell (all 8 neighbors match), all 8
  perimeter alphas are 1.0 and center is 0.0; the fan interpolates
  from 1 at the edge to 0 at the center, producing a radial gradient.
  That's correct.
- **Keep `edge_mask` public API or remove?** It's `pub(crate)` so the
  rename is internal. No external consumers. Delete it.

## 7. Commit shape

One commit on a feature branch `feat/terrain-edges-9vert`:

```
extend terrain edges to 8-neighbor 9-vertex fans

Match RimWorld's SectionLayer_Terrain emission rule: for each overlaid
neighbor def, emit a 9-vertex fan (8 perimeter points + 1 center) with
per-vertex alpha driven by which of the 8 surrounding cells are the
overlay def. Cardinal match lights 3 perimeter verts; diagonal match
lights 1. Triangulation + interpolation produces smooth rounded fades
at convex and concave corners instead of the flat 4-vert quad's
stepped/pinched artifacts.

Also tightens the precedence gate to `>=` (cross-emits equal-precedence
terrains) and skips Hard-edge terrains as overlay sources, both
matching Verse/SectionLayer_Terrain.cs:86.

Shader simplifies: fade shape now lives in the geometry, not in a
per-side smoothstep calculation.
```

If Phase C's renderer refactor grows beyond ~200 lines, split as:

1. helpers + data model (Phases A–B)
2. renderer pipeline + shader (Phases C–D)
3. fixture regen + followups cleanup (Phases E–F)

## 8. Risk ledger

- **Index-buffer off-by-one on fan triangulation**. The 8-tri closure
  `(m+1)%8` is easy to get wrong. Unit-test the index generator as a
  pure function.
- **Vertex ordering drift**: if the neighbor iteration order and the
  perimeter vertex order disagree, alphas land on the wrong verts.
  Nail both to RimWorld's `AdjacentCellsAroundBottom` convention, name
  the constants with the order in the comment, and unit-test with
  hand-built match arrays.
- **UV flip re-enters**: our earlier wall bug was a UV-row flip between
  Unity's bottom-left and wgpu's top-left convention. The fan's uv.y
  for perimeter north verts is 0 (top), south verts is 1 (bottom).
  Keep that consistent with the base terrain quad so sampling the
  neighbor's texture stays aligned.
- **Equal-precedence cross-emit doubling**. Two equal-precedence
  terrains touching will each produce a contribution onto the other.
  Draw order (by depth) must be stable or the z-fight tint will
  flicker. Current sort is `by min_z` with a texture-id tiebreaker;
  leave it.
