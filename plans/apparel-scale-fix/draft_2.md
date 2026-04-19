# Apparel Render Scale Fix -- Implementation Plan (Draft 2)

## Context & Goal

Pawn apparel textures render at ~67% of their correct size. Body and head nodes correctly use `HUMANLIKE_MESH_BASE` (1.5) as their quad size, but apparel nodes use `graphicData.drawSize` (defaults to 1.0) as the quad base instead of the humanlike mesh base. The result is that shirts, jackets, helmets, etc. are visibly too small relative to the body they are supposed to cover.

This plan restructures the apparel size model in `compose.rs` and `ApparelRenderInput` to match RimWorld's actual mesh + ScaleFor separation, and corrects the `draw_scale` semantics to only apply worn-graphic scaling for pack items (matching RimWorld's `BeltScaleAt` behavior).

## Research Findings

### What RimWorld actually does (decompiled)

1. **Mesh selection**: Both body and apparel use the same 1.5x1.5 humanlike mesh via `GetHumanlikeBodySetForPawn()`. The mesh size IS the quad size. `graphicData.drawSize` is only used to create the `Graphic` object (texture atlas selection), not for mesh/quad sizing.

2. **ScaleFor**: `PawnRenderNodeWorker.ScaleFor()` multiplies by `node.Props.drawSize`, which defaults to `Vector2.one` for dynamically-created apparel nodes (never explicitly set in `DynamicPawnRenderNodeSetup_Apparel`). So the effective scale multiplier is 1.0 for standard apparel.

3. **BeltScaleAt exception**: `PawnRenderNodeWorker_Apparel_Body.ScaleFor()` overrides the base ScaleFor only for pack items (`RenderAsPack()`), applying `BeltScaleAt` directional scaling. Non-pack apparel gets no special scaling from wornGraphicData.

### What stitchlands-redux currently does (the bugs)

**Bug 1 -- Wrong quad base for apparel** (`compose.rs:209-211`):
```rust
Vec2::new(
    apparel.draw_size.x * apparel.draw_scale.x,
    apparel.draw_size.y * apparel.draw_scale.y,
)
```
Here `draw_size` comes from `apparel.draw_size` in `fixture_v2_cmd.rs:434`, which is `graphicData.drawSize` (defaults to 1.0). The correct quad base should be `HUMANLIKE_MESH_BASE` (1.5), same as body/head.

**Bug 2 -- draw_scale applied to all apparel** (`fixture_v2_cmd.rs:432`):
```rust
draw_scale: worn_data.scale,
```
This applies `wornGraphicData` directional scale to ALL apparel items. RimWorld only applies this for pack items via `BeltScaleAt`. For non-pack apparel the scale factor should be `Vec2::ONE`.

### Debug log history (iterations 3 and 4)

- **Iteration 3** tried setting apparel to `* 1.5` mesh basis and found "silhouette and proportion look closer." This was directionally correct.
- **Iteration 4** reverted to `1.0` because "real fixture screenshots with 1.5 body/head/apparel basis were consistently oversized." However, the oversizing was likely caused by Bug 2 -- `draw_scale` from wornGraphicData was being applied on top of the 1.5 base, double-scaling non-pack apparel.

This plan differs from iteration 3 by fixing both bugs simultaneously: use 1.5 mesh base AND restrict `draw_scale` to pack-only items. The iteration 3/4 oscillation happened because only one variable was changed at a time.

## Approach

**Separate mesh_base from scale_factor on `ApparelRenderInput`**, mirroring RimWorld's mesh + ScaleFor separation. This means:

1. Replace the current `draw_size` field (which conflates graphicData.drawSize with quad sizing) with a `mesh_size` field that carries the correct humanlike mesh base.
2. Rename `draw_scale` to `scale_factor` and restrict it to pack-only items at the call site.
3. The compose pipeline then computes: `final_size = mesh_size * scale_factor`, which is clean and mirrors RimWorld's `mesh_size * ScaleFor()`.

This keeps compose.rs ignorant of HUMANLIKE_MESH_BASE -- the caller (fixture_v2_cmd.rs) decides the mesh base, which is the right boundary. Head-anchored apparel (overhead/eyecover) also uses the humanlike head mesh, which is the same 1.5 value in RimWorld.

## Implementation Steps

### Step 1: Rename and restructure `ApparelRenderInput` fields

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/model.rs` (lines 62-78)

Change:
```rust
pub struct ApparelRenderInput {
    // ... existing fields ...
    pub draw_scale: Vec2,   // RENAME to scale_factor
    pub draw_size: Vec2,    // RENAME to mesh_size
    // ...
}
```

- `draw_size: Vec2` becomes `mesh_size: Vec2` -- represents the humanlike mesh quad size (the actual rendered quad dimensions before scaling)
- `draw_scale: Vec2` becomes `scale_factor: Vec2` -- represents ScaleFor() output (1.0 for normal apparel, wornGraphicData scale for pack items only)

No new fields needed. This is a pure rename that makes the semantics explicit.

**Dependencies**: None. This is the foundational change.

### Step 2: Update compose.rs to use new field names

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs` (lines 209-211)

Change the apparel size computation from:
```rust
Vec2::new(
    apparel.draw_size.x * apparel.draw_scale.x,
    apparel.draw_size.y * apparel.draw_scale.y,
)
```
To:
```rust
apparel.mesh_size * apparel.scale_factor
```

This is semantically identical (Vec2 component-wise multiply) but reads correctly with the new names: "mesh size scaled by the ScaleFor factor."

**Dependencies**: Step 1.

### Step 3: Update the call site in fixture_v2_cmd.rs

**File**: `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs` (lines 387-434)

In `build_apparel_inputs()`, change the `ApparelRenderInput` construction:

```rust
let render_as_pack = matches!(apparel.layer, ApparelLayerDef::Belt)
    || apparel.worn_graphic.render_utility_as_pack;

// ...

out.push(ApparelRenderInput {
    // ... other fields unchanged ...
    scale_factor: if render_as_pack { worn_data.scale } else { Vec2::ONE },
    mesh_size: Vec2::splat(HUMANLIKE_MESH_BASE),
    // ...
});
```

Key changes:
- `mesh_size` is always `Vec2::splat(HUMANLIKE_MESH_BASE)` (1.5) for all apparel, matching RimWorld's humanlike mesh. This is the same value used for body_size and head_size on line 250-251.
- `scale_factor` is `worn_data.scale` only when `render_as_pack` is true, otherwise `Vec2::ONE`. This matches RimWorld's `BeltScaleAt` behavior.

The `render_as_pack` variable already exists at line 387-388 and is already computed correctly. We just need to use it for the scale gating.

**Dependencies**: Step 1.

### Step 4: Update the public export in pawn/mod.rs

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/mod.rs` (line 11)

No changes needed -- `ApparelRenderInput` is already exported. The field renames are purely internal to the struct.

### Step 5: Update compose.rs unit tests

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs` (lines 260-458)

All three test functions that construct `ApparelRenderInput` need field renames:
- Line 314: `draw_scale: Vec2::ONE` -> `scale_factor: Vec2::ONE`
- Line 316: `draw_size: Vec2::new(1.1, 1.1)` -> `mesh_size: Vec2::new(1.1, 1.1)`
- Line 402: same pattern
- Line 404: same pattern
- Line 418: same pattern
- Line 420: same pattern

The test VALUES stay the same since these are unit tests for ordering/filtering behavior, not size correctness.

**Dependencies**: Step 1.

### Step 6: Update v2 runtime test fixture

**File**: `/Users/jh/personal/stitchlands-redux/src/runtime/v2/mod.rs` (lines 252-278)

The `profile_for()` test helper does not construct `ApparelRenderInput` directly (line 273: `apparel: Vec::new()`), so no changes needed here.

### Step 7: Add a targeted unit test for apparel scale correctness

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs` (append to test module)

Add a new test that verifies the core fix:

```rust
#[test]
fn apparel_node_size_uses_mesh_size_times_scale_factor() {
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
        scale_factor: Vec2::new(0.9, 1.1),
        layer_override: None,
        mesh_size: Vec2::new(1.5, 1.5),
        tint: [1.0, 1.0, 1.0, 1.0],
    });

    let result = compose_pawn(&input, &PawnComposeConfig::default());
    let apparel_node = result
        .nodes
        .iter()
        .find(|n| n.id.contains("Jacket"))
        .expect("jacket node");
    assert!(
        (apparel_node.size.x - 1.35).abs() < 0.001,
        "expected 1.5 * 0.9 = 1.35, got {}",
        apparel_node.size.x
    );
    assert!(
        (apparel_node.size.y - 1.65).abs() < 0.001,
        "expected 1.5 * 1.1 = 1.65, got {}",
        apparel_node.size.y
    );
}
```

This locks in the invariant: apparel quad size = mesh_size * scale_factor.

**Dependencies**: Steps 1, 2.

## Data Flow (after fix)

```
ApparelDef (from XML)
  |
  +-- graphicData.drawSize  --> IGNORED for quad sizing (only used for Graphic object)
  +-- wornGraphicData.scale --> scale_factor (only if render_as_pack, else Vec2::ONE)
  |
  v
ApparelRenderInput {
    mesh_size: Vec2::splat(1.5),     // humanlike mesh base, same as body/head
    scale_factor: Vec2::ONE or pack_scale,
}
  |
  v
compose.rs evaluate_graph()
  final_quad_size = mesh_size * scale_factor
  |
  v
PawnNode { size: final_quad_size }
  |
  v
SpriteParams / GPU quad
```

For comparison, body/head flow:
```
body_size: Vec2::splat(HUMANLIKE_MESH_BASE) --> PawnNode { size: body_size }
head_size: Vec2::splat(HUMANLIKE_MESH_BASE) --> PawnNode { size: head_size }
```

After the fix, all humanlike layers (body, head, apparel) share the same 1.5 mesh base, which is exactly how RimWorld works.

## Testing Strategy

1. **Unit test** (Step 7): Verifies `mesh_size * scale_factor` arithmetic in compose.
2. **Existing unit tests** (Step 5): Continue to verify ordering, filtering, anchoring. Just need field renames.
3. **Existing orientation guardrail** (iteration 9/10): `head_body_delta_y > 0` and z-order checks remain unaffected.
4. **Manual visual validation**: Run the pawn fixture with the 10-variant loop and compare screenshots against RimWorld reference. Apparel should now fill the body silhouette instead of appearing 67% too small.
5. **cargo clippy / cargo test**: Standard CI gates.

No new integration test infrastructure needed. The existing fixture pipeline with trace dumps is sufficient.

## Risks & Unknowns

1. **Head-anchored apparel mesh size**: In RimWorld, overhead/eyecover apparel also uses the humanlike head mesh set (which is also 1.5 for adults). Using `Vec2::splat(HUMANLIKE_MESH_BASE)` is correct for both body-anchored and head-anchored apparel. If non-humanlike pawns are added later, this will need parameterization, but that is not a current concern.

2. **graphicData.drawSize for non-standard apparel**: Some modded apparel might use non-default `graphicData.drawSize` to signal intentionally larger/smaller graphics. Since we are ignoring `graphicData.drawSize` for quad sizing (matching RimWorld), this is correct behavior. The draw_size on `ApparelDef` in `defs.rs:130` can remain parsed but unused by the pawn render pipeline (it is still used by the thing renderer for non-pawn items).

3. **wornGraphicData scale for non-pack items**: Some apparel defs do have non-identity wornGraphicData scale values even when not pack items. In RimWorld, these scale values affect the Graphic object creation but NOT the render node ScaleFor. Our fix (gating on render_as_pack) matches RimWorld. If visual artifacts appear for specific items, the cause will be elsewhere (likely offset, not scale).

4. **Iteration 4 regression concern**: Iteration 4 reverted 1.5 mesh base because apparel was "oversized." With Bug 2 also fixed (pack-only scale), the 1.5 base should produce correctly-sized apparel. But this should be visually validated against RimWorld screenshots to confirm.

## Out of Scope

- **Non-humanlike pawns**: This plan assumes adult humanlike mesh (1.5). Animal or child mesh sizes are not addressed.
- **West mirroring**: Sprite flipping for west-facing is a separate concern (iteration 8/13).
- **Body-type-specific mesh scaling**: RimWorld's `bodyGraphicScale` on BodyTypeDef could scale the body mesh slightly. The existing `body_size_factor` field exists for this but is currently always 1.0. Not addressed here.
- **graphicData.drawSize cleanup**: The `draw_size` field on `ApparelDef` in `defs.rs` is still parsed and stored. Removing it would be a separate cleanup since it may be used for debug/trace output or future non-pawn rendering paths.
- **v1 fixture command**: `fixture_cmd.rs` does not render apparel, so no changes needed there.
