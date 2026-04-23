# Apparel Scale Fix Plan

## Context and Goal

Apparel textures render at roughly 67% of their correct size. The root cause: apparel quad size is driven by `graphicData.drawSize` (which defaults to `1.0x1.0` for most apparel) multiplied by `wornGraphicData` directional scale, when it should instead use the humanlike mesh base (`1.5x1.5`) as the quad foundation -- exactly the same as body and head.

In RimWorld's actual render pipeline, `graphicData.drawSize` is only used to create the `Graphic` object (texture atlas selection); it is **not** used for mesh/quad sizing. Both body and apparel use the same 1.5x1.5 humanlike mesh. The `PawnRenderNodeWorker.ScaleFor()` method multiplies by `node.Props.drawSize`, which defaults to `Vector2.one` for dynamically-created apparel nodes.

The `wornGraphicData` directional scale (`draw_scale` in our code) comes from `PawnRenderNodeWorker_Apparel_Body.ScaleFor()` -- but in RimWorld this **only** applies the belt scale for pack items (`RenderAsPack()`), not for all apparel.

## Research Findings

### Debug log history (iterations 3 and 4)

Iteration 3 applied `* 1.5` to apparel draw size and reported "closer to RimWorld baselines." Iteration 4 then **reverted** body/head/apparel basis to `1.0`, claiming "consistently oversized and less RimWorld-like."

The key mistake in iteration 4: it set **body and head** basis to 1.0 simultaneously with apparel. That made everything uniformly small, so relative proportions looked okay, but absolute scale was wrong. Later work (current code at `fixture_v2_cmd.rs:250-251`) restored body/head to `HUMANLIKE_MESH_BASE` (1.5) but left apparel using `graphicData.drawSize` -- creating the mismatch we see now.

This fix is different from iteration 3's approach because we are **not** multiplying `graphicData.drawSize` by 1.5. We are replacing the draw_size source entirely with the mesh base constant, matching what RimWorld actually does.

### Current data flow

1. `src/defs.rs:754-756` -- `graphicData.drawSize` parsed, defaults to `Vec2::new(1.0, 1.0)`
2. `src/defs.rs:130` -- stored as `ApparelDef.draw_size: Vec2`
3. `src/commands/fixture_v2_cmd.rs:434` -- passed through as `draw_size: apparel.draw_size`
4. `src/commands/fixture_v2_cmd.rs:432` -- `draw_scale: worn_data.scale` (from wornGraphicData directional scale)
5. `src/pawn/compose.rs:209-211` -- final size = `draw_size * draw_scale` (both wrong factors)

### All construction sites for ApparelRenderInput

- `src/commands/fixture_v2_cmd.rs:421` -- production path (the bug site)
- `src/pawn/compose.rs:303` -- test fixture in `full_head_coverage_hides_hair_and_beard`
- `src/pawn/compose.rs:391` -- test fixture in `apparel_sorted_by_layer_draw_order` (Helmet)
- `src/pawn/compose.rs:407` -- test fixture in `apparel_sorted_by_layer_draw_order` (Shirt)

### draw_scale consumers

Only one: `compose.rs:210-211` where it multiplies with `draw_size`.

## Approach

Make two targeted changes:

1. **Remove `draw_size` and `draw_scale` from `ApparelRenderInput`** and replace them with a single approach: apparel always uses `HUMANLIKE_MESH_BASE` as its quad size in compose, matching RimWorld's mesh sizing. The `wornGraphicData` scale (`draw_scale`) should still apply but **only for pack items** (belt layer or `render_utility_as_pack`), matching `PawnRenderNodeWorker_Apparel_Body.ScaleFor()` behavior.

2. **Carry a `render_as_pack` flag** on `ApparelRenderInput` so compose knows whether to apply the worn scale. Non-pack apparel gets a flat `HUMANLIKE_MESH_BASE` quad. Pack apparel gets `HUMANLIKE_MESH_BASE * worn_scale`.

This is the minimal change that fixes the bug, removes dead data (`draw_size`), and makes the scale behavior match RimWorld.

## Implementation Steps

### Step 1: Modify `ApparelRenderInput` struct

**File**: `src/pawn/model.rs:63-78`

- Remove `draw_size: Vec2` (line 76)
- Rename `draw_scale: Vec2` to `pack_scale: Vec2` to clarify its purpose
- Add `render_as_pack: bool` field

```rust
pub struct ApparelRenderInput {
    pub label: String,
    pub tex_path: String,
    pub layer: ApparelLayer,
    pub explicit_skip_hair: bool,
    pub explicit_skip_beard: bool,
    pub has_explicit_skip_flags: bool,
    pub covers_upper_head: bool,
    pub covers_full_head: bool,
    pub anchor_to_head: Option<bool>,
    pub draw_offset: Vec2,
    pub pack_scale: Vec2,       // was draw_scale; only applied when render_as_pack
    pub render_as_pack: bool,   // new
    pub layer_override: Option<f32>,
    // draw_size removed
    pub tint: [f32; 4],
}
```

### Step 2: Update compose apparel size calculation

**File**: `src/pawn/compose.rs:188-216`

Replace lines 209-212 with:

```rust
let base = Vec2::splat(super::model::HUMANLIKE_MESH_BASE);
let size = if apparel.render_as_pack {
    Vec2::new(
        base.x * apparel.pack_scale.x,
        base.y * apparel.pack_scale.y,
    )
} else {
    base
};
```

Then use `size` as the first element of the returned tuple (replacing the old `Vec2::new(draw_size.x * draw_scale.x, ...)` expression).

### Step 3: Update production construction site

**File**: `src/commands/fixture_v2_cmd.rs:421-441`

In `build_apparel_inputs()`, the `render_as_pack` bool is already computed at line 387-388. Pass it through and remove `draw_size`:

```rust
out.push(ApparelRenderInput {
    label: apparel.def_name.clone(),
    tex_path,
    layer: apparel.layer.into(),
    explicit_skip_hair,
    explicit_skip_beard,
    has_explicit_skip_flags,
    covers_upper_head: apparel.covers_upper_head,
    covers_full_head: apparel.covers_full_head,
    anchor_to_head,
    draw_offset: worn_data.offset,
    pack_scale: worn_data.scale,
    render_as_pack,
    layer_override,
    // draw_size removed -- apparel uses HUMANLIKE_MESH_BASE
    tint: [
        apparel.color.r,
        apparel.color.g,
        apparel.color.b,
        apparel.color.a,
    ],
});
```

### Step 4: Update test fixtures in compose.rs

**File**: `src/pawn/compose.rs` -- three test construction sites

For each `ApparelRenderInput` in tests (lines 303, 391, 407):
- Remove `draw_size` field
- Rename `draw_scale` to `pack_scale`
- Add `render_as_pack: false`

The tests don't assert on apparel size values; they test ordering, head coverage, and z-layering. The structural change to the struct fields is all that's needed to keep them compiling.

### Step 5: Update pawn/mod.rs re-exports (if needed)

**File**: `src/pawn/mod.rs:11`

No changes needed -- `ApparelRenderInput` is already re-exported, and we're not adding/removing types from the module boundary.

### Step 6: Consider removing `draw_size` from `ApparelDef`

**File**: `src/defs.rs:130`

`ApparelDef.draw_size` is sourced from `graphicData.drawSize`. It is still used in one place: `fixture_v2_cmd.rs:434` which we're removing. Check for any other consumers:

Searching shows the only consumer of `apparel.draw_size` is the line we're changing. However, the field is parsed from XML and may be useful for future Graphic object creation (texture atlas selection). **Keep it in `ApparelDef`** -- it's a faithful representation of the XML data. Just stop using it for quad sizing.

## Data Flow (after fix)

```
RimWorld XML
    |
    v
ApparelDef.draw_size (kept, not used for quad sizing)
ApparelDef.worn_graphic.{direction}.scale (directional worn scale)
    |
    v
build_apparel_inputs()
    - render_as_pack = Belt layer || render_utility_as_pack
    - pack_scale = worn_data.scale (from wornGraphicData)
    |
    v
ApparelRenderInput { pack_scale, render_as_pack, ... }
    |
    v
compose.rs evaluate_graph()
    - if render_as_pack: size = HUMANLIKE_MESH_BASE * pack_scale
    - else:              size = HUMANLIKE_MESH_BASE
    |
    v
PawnNode { size: Vec2, ... }
```

## Testing Strategy

### Existing tests that need updating (compile fixes only)
- `pawn::compose::tests::full_head_coverage_hides_hair_and_beard` -- update struct fields
- `pawn::compose::tests::apparel_sorted_by_layer_draw_order` -- update struct fields (x2)

These tests don't assert on size, so they just need the field rename/removal to compile.

### New test to add

Add a test in `src/pawn/compose.rs` tests module:

```rust
#[test]
fn apparel_uses_humanlike_mesh_base_size() {
    let mut input = fixture_input();
    input.apparel.push(ApparelRenderInput {
        label: "Jacket".to_string(),
        tex_path: "Things/Apparel/Body/Jacket".to_string(),
        layer: ApparelLayer::Shell,
        explicit_skip_hair: false,
        explicit_skip_beard: false,
        has_explicit_skip_flags: false,
        covers_upper_head: false,
        covers_full_head: false,
        anchor_to_head: None,
        draw_offset: Vec2::ZERO,
        pack_scale: Vec2::ONE,
        render_as_pack: false,
        layer_override: None,
        tint: [1.0, 1.0, 1.0, 1.0],
    });

    let result = compose_pawn(&input, &PawnComposeConfig::default());
    let jacket = result
        .nodes
        .iter()
        .find(|n| n.id.contains("Jacket"))
        .expect("jacket node");
    let expected = Vec2::splat(crate::pawn::model::HUMANLIKE_MESH_BASE);
    assert_eq!(jacket.size, expected, "non-pack apparel should use mesh base");
}

#[test]
fn pack_apparel_applies_worn_scale() {
    let mut input = fixture_input();
    input.apparel.push(ApparelRenderInput {
        label: "PackThing".to_string(),
        tex_path: "Things/Apparel/Belt/PackThing".to_string(),
        layer: ApparelLayer::Belt,
        explicit_skip_hair: false,
        explicit_skip_beard: false,
        has_explicit_skip_flags: false,
        covers_upper_head: false,
        covers_full_head: false,
        anchor_to_head: None,
        draw_offset: Vec2::ZERO,
        pack_scale: Vec2::new(0.8, 0.9),
        render_as_pack: true,
        layer_override: None,
        tint: [1.0, 1.0, 1.0, 1.0],
    });

    let result = compose_pawn(&input, &PawnComposeConfig::default());
    let pack = result
        .nodes
        .iter()
        .find(|n| n.id.contains("PackThing"))
        .expect("pack node");
    let base = crate::pawn::model::HUMANLIKE_MESH_BASE;
    assert!(
        (pack.size.x - base * 0.8).abs() < 0.001
            && (pack.size.y - base * 0.9).abs() < 0.001,
        "pack apparel should scale mesh base by pack_scale"
    );
}
```

### Golden screenshot test

The golden test (`tests/pawn_fixture_golden.rs`) is gated behind `RIMWORLD_ENABLE_SCREENSHOT_GOLDEN` and will need a new golden image after this fix (the old one had undersized apparel). This is expected and desirable.

### Verify

```bash
cargo test
cargo clippy
```

## Risks and Unknowns

1. **Head-anchored apparel (Overhead/EyeCover)**: In RimWorld, head apparel also uses `GetHumanlikeHeadSetForPawn()` which returns a 1.5 mesh for adults. Our fix uses `HUMANLIKE_MESH_BASE` for all apparel regardless of anchor, which is correct for adult humanlike pawns. If child/baby pawns are added later, this will need per-lifestage mesh sizes.

2. **Pack scale fidelity**: RimWorld's `BeltScaleAt` computation involves `mesh.bounds.size` and the pack graphic's `drawSize`. Our approximation (worn directional scale) may not match exactly for every pack item. This is a pre-existing approximation, not introduced by this fix.

3. **Non-default wornGraphicData scale on non-pack items**: Some apparel may have non-`Vec2::ONE` directional scale in their `wornGraphicData` even though they're not packs. After this fix, that scale will be ignored for non-pack items. This matches RimWorld behavior (only pack items get `BeltScaleAt`), but if any hand-authored apparel relied on the old behavior for visual tuning, it will look different.

## Out of Scope

- Child/baby pawn mesh sizes (different from adult 1.5)
- West-facing mirror rendering
- Precise `BeltScaleAt` computation using graphic drawSize and mesh bounds
- Removing `draw_size` from `ApparelDef` (it faithfully represents XML data)
- Any changes to hediff overlay sizing (uses its own `draw_size`, unrelated)
