# Adversarial Review: Apparel Render Scale Fix (Draft 2)

## Plan Summary

The plan fixes two bugs causing apparel to render at ~67% of correct size: (1) apparel quads use `graphicData.drawSize` (default 1.0) instead of the humanlike mesh base (1.5), and (2) `wornGraphicData` directional scale is applied to all apparel instead of only pack items. The fix renames `draw_size`/`draw_scale` to `mesh_size`/`scale_factor`, hardcodes `mesh_size` to `HUMANLIKE_MESH_BASE` (1.5), and gates `scale_factor` behind `render_as_pack`.

## Critical Issues

None. The plan correctly identifies the two bugs, the fix is well-grounded in decompiled RimWorld source, and the implementation steps are mechanically sound.

## Significant Gaps

### 1. BeltOffsetAt is also pack-only, but worn_data.offset is applied to all apparel

The plan correctly fixes the scale gating but does not address an analogous offset bug. In RimWorld, `PawnRenderNodeWorker_Apparel_Body.OffsetFor()` only applies `wornGraphicData.BeltOffsetAt()` when `RenderAsPack()` is true (lines 24-29 of `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker_Apparel_Body.cs`). For non-pack body apparel, the offset comes from `drawData.OffsetForRot()` on the node properties (lines 181-188 of `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker.cs`), NOT from `wornGraphicData`.

Currently in `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs` line 431:
```rust
draw_offset: worn_data.offset,
```
This applies `wornGraphicData` directional offset to ALL apparel. For non-pack apparel, the offset should come from the apparel's `drawData` instead.

**Impact**: This is not introduced by this plan and is a pre-existing bug. However, since the plan is specifically fixing the scale half of the same RimWorld-parity problem in `PawnRenderNodeWorker_Apparel_Body`, the offset half should at least be called out as a known remaining gap. For most vanilla apparel, `wornGraphicData` offsets are zero for non-pack items, so the visual impact is likely minor, but it could cause subtle misalignment for specific items that have non-zero wornGraphicData offsets on non-pack layers.

**Recommendation**: Either fix the offset gating in the same change (gate `draw_offset` on `render_as_pack` too, falling back to drawData offset for non-pack apparel), or explicitly document this as a known remaining gap in the plan's "Out of Scope" section.

### 2. render_as_pack logic diverges from RimWorld's RenderAsPack()

The existing `render_as_pack` computation at `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs` lines 387-388:
```rust
let render_as_pack = matches!(apparel.layer, ApparelLayerDef::Belt)
    || apparel.worn_graphic.render_utility_as_pack;
```

RimWorld's actual `RenderAsPack()` at `/Users/jh/rimworld-decompiled/Verse/PawnRenderUtility.cs` lines 38-47:
```csharp
if (apparel.def.apparel.LastLayer.IsUtilityLayer) // IsUtilityLayer => Belt
{
    if (apparel.def.apparel.wornGraphicData != null)
        return apparel.def.apparel.wornGraphicData.renderUtilityAsPack;
    return true; // default true when no wornGraphicData
}
return false; // non-Belt layers never render as pack
```

The differences:
- **RimWorld**: For Belt items, checks `renderUtilityAsPack` (can be false, disabling pack rendering for Belt items). For non-Belt items, always returns false.
- **Stitchlands**: For Belt items, always true regardless of `renderUtilityAsPack`. For non-Belt items, returns true if `renderUtilityAsPack` is set (which should never happen in valid data, but is structurally wrong).

The plan relies on this variable for its core fix (gating `scale_factor`) but does not address or mention the divergence. The plan even states "The `render_as_pack` variable already exists at line 387-388 and is already computed correctly," which is inaccurate relative to RimWorld's actual logic.

**Impact**: Medium. Belt-layer items with `renderUtilityAsPack: false` would incorrectly get pack-style scaling. This is a rare data pattern but becomes a live bug now that `render_as_pack` gates a visible scaling behavior. Note that the current OR-based logic accidentally handles the "Belt with no wornGraphicData" case correctly (returns true, matching RimWorld's default-true behavior), but for the wrong reason.

Additionally, `/Users/jh/personal/stitchlands-redux/src/defs.rs` line 828 defaults `render_utility_as_pack` to `false` when the XML field is absent, whereas RimWorld defaults to `true` when the entire `wornGraphicData` block is null. The OR logic happens to compensate for this mismatch, but the reasoning is fragile.

**Recommendation**: Add a comment explaining this divergence and why it produces correct results for vanilla data. Optionally fix to match RimWorld's actual three-way logic.

## Incorrect Assumptions

### 1. "render_as_pack is already computed correctly" -- inaccurate

As detailed in Significant Gap #2. The plan at line 120 claims this variable "is already computed correctly." It produces correct results for common vanilla data patterns, but the logic structure does not match `RenderAsPack()`. Since the plan increases the load this variable bears, the claim warrants correction.

### 2. Head-anchored apparel mesh size claim -- verified correct

The plan claims head-anchored apparel uses the humanlike head mesh, which is also 1.5 for adults. Verified at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNode_Apparel.cs` lines 33-48: `MeshSetFor()` returns `GetHumanlikeHeadSetForPawn()` when `useHeadMesh` is true, and `GetHumanlikeBodySetForPawn()` otherwise. Both return 1.5 for adults per `/Users/jh/rimworld-decompiled/Verse/HumanlikeMeshPoolUtility.cs` lines 7-14 and 16-24. Using `Vec2::splat(HUMANLIKE_MESH_BASE)` for all apparel is correct.

### 3. PawnRenderNodeWorker_Apparel_Head does NOT override ScaleFor -- verified correct

The head apparel worker at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker_Apparel_Head.cs` extends `PawnRenderNodeWorker_FlipWhenCrawling` (at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker_FlipWhenCrawling.cs`) which extends `PawnRenderNodeWorker` -- no ScaleFor override in either class. So head apparel gets the base `ScaleFor()` which returns ~Vec3.one for default `drawSize`. Consistent with the plan's approach.

### 4. Base ScaleFor returns Vec3.one for dynamically-created apparel -- verified correct

`PawnRenderNodeProperties.drawSize` defaults to `Vector2.one` (line 90 of `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeProperties.cs`). The `DynamicPawnRenderNodeSetup_Apparel` at `/Users/jh/rimworld-decompiled/Verse/DynamicPawnRenderNodeSetup_Apparel.cs` lines 91-114 never sets `drawSize` on the dynamically-created properties. So `PawnRenderNodeWorker.ScaleFor()` at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker.cs` lines 250-272 multiplies by 1.0, returning effectively Vec3.one. This confirms the plan's claim.

## Risks

### 1. Iteration 3/4 regression (Low likelihood, Low impact)

The plan correctly identifies that iterations 3 and 4 oscillated because they changed one variable at a time (see `/Users/jh/personal/stitchlands-redux/docs/pawn-render-debug-log.md` iterations 3-4). This plan changes both simultaneously (1.5 mesh base + pack-only scaling). The analysis is sound: with Bug 2 also fixed, the 1.5 base should not produce the oversizing seen in iteration 4. The existing guardrails (`head_body_delta_y > 0`, z-order checks from iterations 9-11) provide automated regression detection.

### 2. Compose.rs remaining ignorant of HUMANLIKE_MESH_BASE (No risk)

The plan keeps `compose.rs` unaware of `HUMANLIKE_MESH_BASE`. This is the right boundary -- compose just multiplies `mesh_size * scale_factor`, and the caller decides the mesh base. No problems with this design.

### 3. The rename may cause naming asymmetry with HediffOverlayInput (Low risk)

After the rename, `ApparelRenderInput` will use `mesh_size` while the sibling struct `HediffOverlayInput` (at `/Users/jh/personal/stitchlands-redux/src/pawn/model.rs` line 141) still uses `draw_size`. These fields serve different semantic purposes (hediff overlays DO use `graphicData.drawSize` for sizing), so the asymmetry is actually correct. But someone reading the code might wonder why they differ. A brief comment on either struct would help. Not a blocking issue.

### 4. Test values in existing tests remain at 1.1/1.3 (No risk)

The plan correctly notes that existing test VALUES for `draw_size` -> `mesh_size` (1.1, 1.3) should remain unchanged since those tests verify ordering/filtering behavior, not size correctness. The new Step 7 test uses 1.5. This is fine.

## Recommendations

1. **Fix or document the offset gating gap**: The `draw_offset: worn_data.offset` at `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs` line 431 should either be gated on `render_as_pack` (with drawData offset used for non-pack apparel), or explicitly listed in "Out of Scope" with a rationale. Since `PawnRenderNodeWorker_Apparel_Body.OffsetFor()` at `/Users/jh/rimworld-decompiled/Verse/PawnRenderNodeWorker_Apparel_Body.cs` lines 20-31 applies the same `RenderAsPack()` gate pattern for offsets as it does for scale, fixing both in one change would be the most coherent approach.

2. **Add a comment explaining the render_as_pack logic divergence**: Even if not fixed now, the `render_as_pack` computation at `/Users/jh/personal/stitchlands-redux/src/commands/fixture_v2_cmd.rs` lines 387-388 should have a comment noting the semantic difference from RimWorld's `RenderAsPack()` and why it still produces correct results for vanilla data. Correct the plan's claim that it is "already computed correctly."

3. **The plan is otherwise ready to implement**: The core fix (mesh_size = 1.5, scale_factor gated on render_as_pack) is correct and well-researched. The rename improves clarity. The new unit test locks in the invariant. All file references and line numbers check out against the actual codebase. The iteration 3/4 regression analysis is credible and the existing test guardrails mitigate the risk.
