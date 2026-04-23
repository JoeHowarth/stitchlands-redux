# Apparel Scale Fix -- Synthesized Implementation Plan

## Sources

- `./plans/apparel-scale-fix/draft_1.md` -- simplicity-focused plan (remove draw_size, add render_as_pack bool)
- `./plans/apparel-scale-fix/draft_2.md` -- extensibility-focused plan (rename draw_size->mesh_size, draw_scale->scale_factor)
- `./plans/apparel-scale-fix/adversarial_1.md` -- review of draft 1
- `./plans/apparel-scale-fix/adversarial_2.md` -- review of draft 2

## Synthesis Rationale

### Decision 1: Remove draw_size (Draft 1) vs rename to mesh_size (Draft 2)

**Choice: Remove draw_size from ApparelRenderInput (Draft 1 approach).**

Draft 2 renames `draw_size` to `mesh_size` and has the caller pass `Vec2::splat(HUMANLIKE_MESH_BASE)`. But `mesh_size` would always be set to the same constant for all adult humanlike apparel -- it carries no information. Keeping a field that is always `Vec2::splat(1.5)` is dead weight that makes compose.rs look like it handles variable mesh sizes when it does not. Draft 1's approach is better: hardcode `HUMANLIKE_MESH_BASE` directly in compose.rs where the size is computed, since that is the actual invariant ("all humanlike apparel uses the same mesh base"). If non-humanlike or child pawns are added later, that is when the field should be introduced -- not before.

### Decision 2: Rename draw_scale to pack_scale (Draft 1) vs scale_factor (Draft 2)

**Choice: Rename to pack_scale (Draft 1 approach) and add render_as_pack bool.**

Draft 2 renames to `scale_factor` and gates it at the call site (`if render_as_pack { worn_data.scale } else { Vec2::ONE }`). This pushes pack awareness into the call site but then compose.rs blindly multiplies, which means compose cannot distinguish pack from non-pack items for offset gating (see Decision 3). Draft 1's `render_as_pack` bool on the struct gives compose the information it needs to gate both scale AND offset correctly.

### Decision 3: Gate draw_offset on render_as_pack (Adversarial finding)

**Choice: Yes, gate it. Both adversarial reviews independently flagged this.**

RimWorld's `PawnRenderNodeWorker_Apparel_Body.OffsetFor()` gates `BeltOffsetAt` behind the same `RenderAsPack()` check as the scale. The current code applies `worn_data.offset` unconditionally to all apparel (line 214 of compose.rs). Since we are already restructuring the pack-awareness on this struct, fixing the offset gating in the same change is the right call. Most non-pack vanilla apparel has zero offsets so this is unlikely to cause visual regressions, but it is correct.

Rename `draw_offset` to `pack_offset` to match `pack_scale` naming, and gate both on `render_as_pack` in compose.rs.

### Decision 4: Fix render_as_pack boolean logic (Adversarial finding)

**Choice: Fix it, but carefully.**

Both adversarial reviews flagged that the current OR-based logic at `fixture_v2_cmd.rs:387-388` diverges from RimWorld's `RenderAsPack()`. Now that `render_as_pack` gates scale and offset behavior (not just texture path), getting this right matters more.

The correct RimWorld logic is: Belt items check `renderUtilityAsPack` (defaulting to true when no wornGraphicData exists); non-Belt items always return false. The complication is the parser default: `render_utility_as_pack` defaults to `false` at `defs.rs:828-830` when the XML tag is absent, but RimWorld returns `true` for Belt items with no `wornGraphicData` at all. The OR-based logic accidentally compensated for this.

The fix: for Belt items, use `render_utility_as_pack`. The parser default of `false` is technically wrong for Belt items that lack a `wornGraphicData` XML node entirely, since RimWorld would return `true`. But all vanilla Core Belt items have explicit `wornGraphicData` with `renderUtilityAsPack` set. The edge case only affects malformed modded content.

```rust
let render_as_pack = if matches!(apparel.layer, ApparelLayerDef::Belt) {
    apparel.worn_graphic.render_utility_as_pack
} else {
    false
};
```

### Decision 5: Test strategy

**Choice: Combine the best from both plans.**

Draft 1's two separate tests (one for non-pack size, one for pack scale) are better structured than Draft 2's single test that uses non-default scale on a non-pack item (which is misleading since non-pack items should have `scale_factor: Vec2::ONE`). The non-pack test should carry non-default pack_scale and pack_offset values to prove they are ignored.

## The Plan

### Context & Goal

Apparel textures render at ~67% correct size. Two bugs:

1. Apparel quad size uses `graphicData.drawSize` (defaults to 1.0) instead of the humanlike mesh base (1.5) that body and head use.
2. `wornGraphicData` directional scale and offset are applied to ALL apparel, when RimWorld only applies them for pack items (`RenderAsPack()`).

After this fix, all humanlike apparel uses `HUMANLIKE_MESH_BASE` (1.5) as quad base, and worn scale/offset from `wornGraphicData` only apply to pack items.

### Implementation Steps

#### Step 1: Modify `ApparelRenderInput` struct

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/model.rs`, lines 62-78

Remove `draw_size` (line 76). Rename `draw_scale` (line 74) to `pack_scale`. Rename `draw_offset` (line 73) to `pack_offset`. Add `render_as_pack: bool`.

```rust
#[derive(Debug, Clone)]
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
    pub pack_offset: Vec2,       // was draw_offset; only applied when render_as_pack
    pub pack_scale: Vec2,        // was draw_scale; only applied when render_as_pack
    pub render_as_pack: bool,    // new: gates pack_offset and pack_scale
    pub layer_override: Option<f32>,
    // draw_size removed -- apparel uses HUMANLIKE_MESH_BASE directly
    pub tint: [f32; 4],
}
```

#### Step 2: Update compose.rs apparel size and offset calculation

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs`, lines 207-216

Replace the current apparel tuple construction (lines 207-216):

```rust
(
    apparel.tex_path.clone(),
    Vec2::new(
        apparel.draw_size.x * apparel.draw_scale.x,
        apparel.draw_size.y * apparel.draw_scale.y,
    ),
    apparel.tint,
    workers::apparel_offset(apparel.layer, config.layering) + apparel.draw_offset,
    z,
)
```

With:

```rust
let base = Vec2::splat(super::model::HUMANLIKE_MESH_BASE);
let size = if apparel.render_as_pack {
    base * apparel.pack_scale
} else {
    base
};
let extra_offset = workers::apparel_offset(apparel.layer, config.layering)
    + if apparel.render_as_pack {
        apparel.pack_offset
    } else {
        Vec2::ZERO
    };
(
    apparel.tex_path.clone(),
    size,
    apparel.tint,
    extra_offset,
    z,
)
```

#### Step 3: Fix render_as_pack boolean logic and update call site

**File**: `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs`

Change the `render_as_pack` computation at lines 387-388 from:

```rust
let render_as_pack = matches!(apparel.layer, ApparelLayerDef::Belt)
    || apparel.worn_graphic.render_utility_as_pack;
```

To:

```rust
// Match RimWorld's RenderAsPack(): only Belt items can render as pack,
// gated by renderUtilityAsPack. Non-Belt items never render as pack.
let render_as_pack = if matches!(apparel.layer, ApparelLayerDef::Belt) {
    apparel.worn_graphic.render_utility_as_pack
} else {
    false
};
```

Then update the `ApparelRenderInput` construction at lines 421-441:

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
    pack_offset: worn_data.offset,
    pack_scale: worn_data.scale,
    render_as_pack,
    layer_override,
    // draw_size removed -- apparel uses HUMANLIKE_MESH_BASE in compose
    tint: [
        apparel.color.r,
        apparel.color.g,
        apparel.color.b,
        apparel.color.a,
    ],
});
```

Note: `pack_offset` and `pack_scale` still carry the worn_data values unconditionally. The gating happens in compose.rs based on `render_as_pack`. This keeps the call site simple and the data available for debugging/tracing.

#### Step 4: Check other consumers of render_as_pack

The variable `render_as_pack` is also used on lines 391-397 (`build_apparel_tex_path`) and line 413 (`build_full_apparel_layer_override`). These call sites use it for texture path and layer override decisions. The new `if/else` logic changes what `render_as_pack` evaluates to for two edge cases:

1. Non-Belt item with `render_utility_as_pack: true` -- was `true`, now `false`. These items do not exist in vanilla Core data.
2. Belt item with `render_utility_as_pack: false` -- was `true` (from Belt match), now `false`. This is the correct fix per RimWorld.

Both edge cases are rare/nonexistent in vanilla data. The texture path and layer override behavior changes are also correct for these cases. No code changes needed beyond Step 3.

#### Step 5: Update test fixtures in compose.rs

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs`

Three test construction sites need field changes:

**Line 303** (MarineHelmet in `full_head_coverage_hides_hair_and_beard`):
```rust
input.apparel.push(ApparelRenderInput {
    label: "MarineHelmet".to_string(),
    tex_path: "Things/Apparel/Headgear/MarineHelmet".to_string(),
    layer: ApparelLayer::Overhead,
    explicit_skip_hair: false,
    explicit_skip_beard: false,
    has_explicit_skip_flags: false,
    covers_upper_head: false,
    covers_full_head: true,
    anchor_to_head: None,
    pack_offset: Vec2::ZERO,
    pack_scale: Vec2::ONE,
    render_as_pack: false,
    layer_override: None,
    tint: [1.0, 1.0, 1.0, 1.0],
});
```

**Line 391** (Helmet in `apparel_sorted_by_layer_draw_order`):
```rust
ApparelRenderInput {
    label: "Helmet".to_string(),
    tex_path: "Things/Apparel/Headgear/SimpleHelmet".to_string(),
    layer: ApparelLayer::Overhead,
    explicit_skip_hair: false,
    explicit_skip_beard: false,
    has_explicit_skip_flags: false,
    covers_upper_head: true,
    covers_full_head: false,
    anchor_to_head: None,
    pack_offset: Vec2::ZERO,
    pack_scale: Vec2::ONE,
    render_as_pack: false,
    layer_override: None,
    tint: [1.0, 1.0, 1.0, 1.0],
},
```

**Line 407** (Shirt in `apparel_sorted_by_layer_draw_order`):
```rust
ApparelRenderInput {
    label: "Shirt".to_string(),
    tex_path: "Things/Apparel/Body/Shirt".to_string(),
    layer: ApparelLayer::OnSkin,
    explicit_skip_hair: false,
    explicit_skip_beard: false,
    has_explicit_skip_flags: false,
    covers_upper_head: false,
    covers_full_head: false,
    anchor_to_head: None,
    pack_offset: Vec2::ZERO,
    pack_scale: Vec2::ONE,
    render_as_pack: false,
    layer_override: None,
    tint: [1.0, 1.0, 1.0, 1.0],
},
```

These tests assert on ordering, coverage flags, and z-layering -- not on size or offset values. The structural changes are sufficient.

#### Step 6: Add new tests

**File**: `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs`, append to `mod tests` block (before the final `}` on line 458)

```rust
#[test]
fn non_pack_apparel_uses_humanlike_mesh_base_size() {
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
        pack_offset: Vec2::new(0.1, 0.2), // non-zero to prove it is ignored
        pack_scale: Vec2::new(0.8, 0.9),  // non-one to prove it is ignored
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
    assert_eq!(jacket.size, expected, "non-pack apparel should use mesh base unscaled");
}

#[test]
fn pack_apparel_applies_worn_scale_and_offset() {
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
        pack_offset: Vec2::new(0.1, 0.2),
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
        "pack apparel should scale mesh base by pack_scale, got {:?}",
        pack.size,
    );
}
```

### Data Flow (after fix)

```
RimWorld XML
    |
    v
ApparelDef
  +-- graphicData.drawSize      --> kept on ApparelDef, NOT used for quad sizing
  +-- wornGraphicData.scale     --> carried as pack_scale on ApparelRenderInput
  +-- wornGraphicData.offset    --> carried as pack_offset on ApparelRenderInput
  +-- renderUtilityAsPack       --> used to compute render_as_pack (Belt items only)
    |
    v
fixture_v2_cmd.rs :: build_apparel_inputs()
  - render_as_pack = Belt && renderUtilityAsPack
  - pack_scale = worn_data.scale (always carried, gated in compose)
  - pack_offset = worn_data.offset (always carried, gated in compose)
    |
    v
ApparelRenderInput { pack_scale, pack_offset, render_as_pack, ... }
    |
    v
compose.rs :: evaluate_graph()
  - if render_as_pack:
      size   = HUMANLIKE_MESH_BASE * pack_scale
      offset = apparel_offset(layer) + pack_offset
  - else:
      size   = HUMANLIKE_MESH_BASE
      offset = apparel_offset(layer)
    |
    v
PawnNode { size, world_pos (incorporating offset), ... }
```

For comparison, body/head:
```
body_size = Vec2::splat(HUMANLIKE_MESH_BASE) --> PawnNode { size: body_size }
head_size = Vec2::splat(HUMANLIKE_MESH_BASE) --> PawnNode { size: head_size }
```

All humanlike layers share the same 1.5 mesh base, matching RimWorld.

### Testing Strategy

1. **Compile-fix existing tests** (Step 5): 3 test construction sites get field renames. No assertion changes needed since they test ordering/coverage, not size.

2. **New unit tests** (Step 6): Two tests locking in the core invariants:
   - Non-pack apparel gets `HUMANLIKE_MESH_BASE` with no scale/offset applied (even when pack_scale/pack_offset are non-default on the struct).
   - Pack apparel gets `HUMANLIKE_MESH_BASE * pack_scale` with offset applied.

3. **Existing guardrails**: `head_body_delta_y > 0` and z-order tests remain unaffected and continue protecting against head/body positional regressions.

4. **Cargo gates**: `cargo test && cargo clippy` must pass clean.

5. **Visual validation**: After the fix, run the pawn fixture with the 10-variant loop and compare against RimWorld reference screenshots. The golden test (`tests/pawn_fixture_golden.rs`) will need a new baseline image (the old one had undersized apparel). This is expected and desirable.

### Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Iteration 3/4 regression (apparel too large) | Low | Medium | This fix differs fundamentally: we replace drawSize with constant 1.5 AND restrict worn scale to pack-only. The iteration 3/4 oscillation happened because only one variable was changed at a time. Visual validation step confirms. |
| Belt item with no wornGraphicData defaults to render_as_pack=false (diverges from RimWorld default true) | Very Low | Low | All vanilla Core Belt items have explicit wornGraphicData with renderUtilityAsPack set. Malformed modded content is the only case. Documented in code comment. |
| Non-pack apparel with non-zero wornGraphicData offset now ignored | Very Low | Low | Matches RimWorld behavior. Most non-pack vanilla apparel has zero offsets. If any hand-tuned offsets existed for visual positioning, they were wrong per RimWorld model. |
| Head-anchored apparel mesh size | None | None | Verified via RimWorld decompiled source: head apparel also uses 1.5 mesh for adults. HUMANLIKE_MESH_BASE is correct for all apparel types. |

## Addressed Concerns

### Adversarial Finding 1 (both reviews): draw_offset should be pack-only gated
**Addressed.** Renamed to `pack_offset` and gated on `render_as_pack` in compose.rs (Step 2). Non-pack apparel gets `Vec2::ZERO` for the worn offset component.

### Adversarial Finding 2 (both reviews): render_as_pack OR-based logic diverges from RimWorld
**Addressed.** Fixed to match RimWorld's `RenderAsPack()` structure: non-Belt always false, Belt checks `render_utility_as_pack` (Step 3). The parser default edge case for Belt items with no wornGraphicData is documented but accepted as a near-zero-probability risk.

### Adversarial Finding 3 (adversarial_1): No test for offset gating
**Addressed.** The `non_pack_apparel_uses_humanlike_mesh_base_size` test uses non-zero `pack_offset` and `pack_scale` to prove they are ignored for non-pack items. The `pack_apparel_applies_worn_scale_and_offset` test verifies pack items get the scale applied.

### Adversarial Finding 4 (adversarial_1): No test for head-anchored apparel size
**Accepted risk, not addressed.** The implementation treats all apparel uniformly with `HUMANLIKE_MESH_BASE`. Adding a test for Overhead-layer apparel would be testing the same code path with a different enum variant but identical logic. If head apparel ever needs a different size (children, animals), that feature would introduce the parameterization and the test simultaneously.

## Accepted Risks

1. **Belt items with no wornGraphicData node**: Our parser defaults `render_utility_as_pack` to false, so these would get `render_as_pack = false` instead of RimWorld's `true`. No vanilla Core Belt items are affected. Fixing this properly would require tracking whether a wornGraphicData XML node was present vs absent, which is not worth the complexity for this edge case.

2. **Base PawnRenderNodeWorker.ScaleFor applies drawData.ScaleFor**: For dynamically-created apparel nodes, `drawData` is typically null (so the scale factor is 1.0). Custom apparel with `drawData.scale != 1` is an extreme edge case not addressed here.

3. **Non-humanlike pawns**: This plan assumes adult humanlike mesh (1.5). Animal or child mesh sizes would need a mesh_size parameter on ApparelRenderInput, which should be introduced when that feature is built.

4. **graphicData.drawSize stays on ApparelDef**: The field remains parsed from XML in `defs.rs:130`. It faithfully represents the XML data and may be useful for future Graphic object creation or debug output. It is just no longer used for quad sizing.
