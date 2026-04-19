# Adversarial Review: Apparel Scale Fix Plan (draft_1)

## Plan Summary

The plan replaces the current apparel sizing logic (`graphicData.drawSize * wornGraphicData.scale`) with `HUMANLIKE_MESH_BASE` (1.5) as the base quad size for all apparel, applying worn scale only for pack items. It removes `draw_size` and renames `draw_scale` to `pack_scale` on `ApparelRenderInput`, adding a `render_as_pack` bool field.

## Critical Issues

### 1. draw_offset is also pack-only in RimWorld but the plan applies it to all apparel

In RimWorld, `PawnRenderNodeWorker_Apparel_Body.OffsetFor` (at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker_Apparel_Body.cs:20-29`) gates the worn graphic offset behind the same `RenderAsPack()` check as the scale:

```csharp
if (pawnRenderNode_Apparel.apparel.def.apparel.wornGraphicData != null && pawnRenderNode_Apparel.apparel.RenderAsPack())
{
    Vector2 vector = pawnRenderNode_Apparel.apparel.def.apparel.wornGraphicData.BeltOffsetAt(parms.facing, parms.pawn.story.bodyType);
    result.x += vector.x;
    result.z += vector.y;
}
```

The plan fixes scale gating but leaves `draw_offset` (which is `worn_data.offset` from `wornGraphicData`) applied unconditionally to all apparel at `/Users/jh/personal/stitchlands-redux/src/pawn/compose.rs:214`:

```rust
workers::apparel_offset(apparel.layer, config.layering) + apparel.draw_offset,
```

For non-pack apparel, `BeltOffsetAt` values should not be applied. Most non-pack items have zero offsets in practice, but if any non-pack item has non-zero directional offsets in `wornGraphicData`, they will be incorrectly applied. This is a pre-existing bug but since the plan is already touching `draw_scale` semantics with the same pack-gating logic, the offset should get the same treatment for consistency and correctness.

**Recommendation**: Gate `draw_offset` with `render_as_pack` the same way `pack_scale` is gated. Rename it to `pack_offset` or add the conditional in compose:

```rust
let extra_offset = if apparel.render_as_pack { apparel.draw_offset } else { Vec2::ZERO };
```

## Significant Gaps

### 2. New tests do not cover the existing apparel offset behavior interaction

The two new tests (`apparel_uses_humanlike_mesh_base_size` and `pack_apparel_applies_worn_scale`) only assert on `size`. They do not verify that `draw_offset` is correctly gated. If the offset fix from Issue 1 is adopted, a test should confirm non-pack apparel ignores worn offsets.

### 3. No test for head-anchored apparel (Overhead/EyeCover) size

The plan's new test (`apparel_uses_humanlike_mesh_base_size`) uses `ApparelLayer::Shell` for the non-pack test. It does not verify that head-anchored apparel (Overhead, EyeCover) also gets `HUMANLIKE_MESH_BASE`. In RimWorld, head apparel uses `GetHumanlikeHeadSetForPawn` (at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNode_Apparel.cs:43-47`) which for adults is also 1.5, so the behavior is correct, but having a test for it would guard against future changes that might try to give head apparel a different size. This is minor since the implementation treats all apparel uniformly.

### 4. Plan does not address the render_as_pack logic mismatch with RimWorld

The existing `render_as_pack` computation at `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs:387-388`:

```rust
let render_as_pack = matches!(apparel.layer, ApparelLayerDef::Belt)
    || apparel.worn_graphic.render_utility_as_pack;
```

differs from RimWorld's `RenderAsPack` at `/Users/jh/rimworld-decompiled/Verse/PawnRenderUtility.cs:38-48`:

```csharp
public static bool RenderAsPack(this Apparel apparel)
{
    if (apparel.def.apparel.LastLayer.IsUtilityLayer) // IsUtilityLayer == (this == Belt)
    {
        if (apparel.def.apparel.wornGraphicData != null)
            return apparel.def.apparel.wornGraphicData.renderUtilityAsPack;
        return true;
    }
    return false;
}
```

The difference: in RimWorld, `renderUtilityAsPack` is only checked when the layer IS Belt. A non-Belt item with `renderUtilityAsPack = true` in XML would NOT render as pack in RimWorld, but WOULD in stitchlands. Also, a Belt item with `wornGraphicData` where `renderUtilityAsPack` is explicitly false gets `render_as_pack = true` from the Belt match arm in stitchlands (incorrect -- should be false per RimWorld).

This is a **pre-existing** bug not introduced by this plan, but since the plan now makes `render_as_pack` gate the scale behavior (previously it was a texture/layer concern only), the impact of this bug increases. A Belt item with `renderUtilityAsPack = false` would get incorrectly scaled.

**Recommendation**: Fix the `render_as_pack` logic to match RimWorld. The correct logic is:

```rust
let render_as_pack = if matches!(apparel.layer, ApparelLayerDef::Belt) {
    apparel.worn_graphic.render_utility_as_pack
} else {
    false
};
```

However, this interacts with the parser default: at `/Users/jh/personal/stitchlands-redux/src/defs.rs:828-830`, `render_utility_as_pack` defaults to `false` when the XML tag is absent. In RimWorld, when `wornGraphicData` is entirely null for a Belt item, `RenderAsPack` returns `true`. So Belt items without any `wornGraphicData` node need `render_utility_as_pack` to default to `true`. The current parser default of `false` means such items would be wrong. Check whether any Core Belt items lack `wornGraphicData` entirely; if they do, the parser default needs to flip or a `has_worn_graphic_data` flag is needed.

## Incorrect Assumptions

### 5. Plan line numbers are slightly off but still reference correct code

The plan references `src/pawn/model.rs:63-78` for `ApparelRenderInput`. The actual struct declaration starts at line 62 (`#[derive(Debug, Clone)]`) with the struct body at line 63. The field `draw_size` is at line 76 and `draw_scale` is at line 74. These are close enough to not cause confusion during implementation.

The plan says "Replace lines 209-212" in compose.rs. The actual code at lines 209-211 is the size computation (`Vec2::new(apparel.draw_size.x * apparel.draw_scale.x, apparel.draw_size.y * apparel.draw_scale.y)`). This is accurate.

### 6. Plan correctly identifies mesh sizing for both body and head apparel

Verified via `/Users/jh/rimworld-decompiled/Verse/PawnRenderNode_Apparel.cs:43-47`: head apparel uses `GetHumanlikeHeadSetForPawn` which returns 1.5 for adults (from `/Users/jh/rimworld-decompiled/Verse/HumanlikeMeshPoolUtility.cs:17-24`), and body apparel uses `GetHumanlikeBodySetForPawn` which also returns 1.5 (from `/Users/jh/rimworld-decompiled/Verse/HumanlikeMeshPoolUtility.cs:7-14`). The plan's uniform `HUMANLIKE_MESH_BASE` is correct for adults.

## Risks

### Risk 1: Iteration 3/4 regression recurrence
**Likelihood**: Low. **Impact**: Medium.

The plan's approach is fundamentally different from iteration 3. Iteration 3 multiplied `graphicData.drawSize * 1.5`, which for items with non-default drawSize (e.g., 2.0) would produce oversized results (3.0). This plan replaces drawSize entirely with the constant 1.5, matching RimWorld's actual mesh behavior. The iteration 4 revert happened because body/head/apparel were ALL set to 1.0 simultaneously; now body/head are already at 1.5 (lines 250-251 of `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs`), so adding apparel at 1.5 should produce correct proportions.

### Risk 2: Base PawnRenderNodeWorker.ScaleFor applies drawData.ScaleFor which we ignore
**Likelihood**: Low. **Impact**: Low.

The base `PawnRenderNodeWorker.ScaleFor` at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker.cs:250-272` also calls `node.Props.drawData.ScaleFor(pawn)` (line 269) which can apply body-type-specific scale. For dynamically-created apparel nodes in `DynamicPawnRenderNodeSetup_Apparel`, `drawData` is typically null or contains only layer overrides (not scale data), so this factor is 1.0 for the vast majority of apparel. Custom apparel with `drawData.scale != 1` is an edge case not addressed here but is correctly out of scope.

### Risk 3: Removing draw_size from ApparelRenderInput may need reversal
**Likelihood**: Very Low. **Impact**: Low.

The plan correctly notes that `graphicData.drawSize` is used for Graphic object creation in RimWorld, not for mesh sizing. The field remains on `ApparelDef` in `/Users/jh/personal/stitchlands-redux/src/defs.rs:130`. If a future feature needs it in compose (unlikely given RimWorld's architecture), it can be re-added. This is explicitly out of scope and well-reasoned.

## Recommendations

1. **Gate `draw_offset` with `render_as_pack`** (Critical, same as Issue 1). The worn offset from `wornGraphicData` (`BeltOffsetAt`) is only applied for pack items in RimWorld. Apply the same pack-only gating to `draw_offset` as to `pack_scale`. Consider renaming it to `pack_offset` and applying conditionally in compose.

2. **Fix `render_as_pack` boolean logic** (Significant). The OR logic at `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs:387-388` does not match RimWorld's `RenderAsPack`. Since this plan elevates `render_as_pack` to gate scale behavior, getting this right matters more now. At minimum, change it so non-Belt items never get `render_as_pack = true` regardless of `renderUtilityAsPack` in their XML.

3. **Visual validation before merging**. Given the iteration 3/4 history, the plan should include a concrete step to regenerate deterministic 10-variant screenshots and visually compare against RimWorld at the same pawn configurations. The plan mentions the golden test needing a new baseline image but doesn't describe the manual comparison step.

4. **Consider a test for pack offset gating** if recommendation 1 is adopted. The proposed `pack_apparel_applies_worn_scale` test can be extended to also assert that the pack item's world position includes the offset, while the non-pack test asserts the offset is zero.
