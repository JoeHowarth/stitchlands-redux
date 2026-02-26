# RimWorld pawn rendering research (decompiled code)

Scope: how pawn rendering composes body/head/hair/beard/apparel, and how body parts affect what is drawn.

Codebase inspected: `/Users/jh/rimworld-decompiled`.

## 1) High-level pipeline

1. `PawnRenderer` builds draw parameters and flags (`PawnDrawParms`) and decides cached vs live draw.
   - See `Verse/PawnRenderer.cs` lines ~303-347, ~490-508.
2. Live draw goes through `PawnRenderTree.ParallelPreDraw(...)` to build `drawRequests`, then `PawnRenderTree.Draw(...)` to submit meshes/materials.
   - See `Verse/PawnRenderTree.cs` lines ~136-167 and ~170-196.
3. A `PawnRenderNode` tree drives composition; each node contributes a graphic, transform, layer, and visibility logic via its worker.
   - See `Verse/PawnRenderNode.cs`, `Verse/PawnRenderNodeWorker.cs`.

## 2) Render tree setup and dynamic nodes

- Race controls the base tree via `RaceProperties.renderTree` (`PawnRenderTreeDef`).
  - `Verse/RaceProperties.cs:68`, `Verse/PawnRenderTreeDef.cs`.
- Tree setup:
  - Create root node from `pawn.RaceProps.renderTree.root`.
  - Run all `DynamicPawnRenderNodeSetup` subclasses.
  - Attach dynamic children by parent tag.
  - Build ancestor cache for matrix composition.
  - See `Verse/PawnRenderTree.cs` lines ~327-367 and ~375-418.

Dynamic node sources relevant here:
- Apparel: `DynamicPawnRenderNodeSetup_Apparel`.
- Hediffs/body-part visuals: `DynamicPawnRenderNodeSetup_Hediffs`.
- Also genes/traits can add overlays/parts: `DynamicPawnRenderNodeSetup_Genes`, `..._Traits`.

## 3) Visibility gating (what draws vs skips)

Global per-node gating happens in `PawnRenderNodeWorker.CanDrawNow(...)`:
- Rot draw mode / facing checks.
- `skipFlag` checks against `PawnDrawParms.skipFlags`.
- Body-part specific conditions:
  - `bodyPart.visibleHediffRots` facing restriction.
  - `linkedBodyPartsGroup` requires at least one non-missing matching part.
- See `Verse/PawnRenderNodeWorker.cs` lines ~18-57.

Node traversal behavior:
- If a node worker draws any request and `props.useGraphic` is true, children are not traversed.
- If node does not produce a request (or `useGraphic=false`), children may draw.
- See `Verse/PawnRenderNode.cs` lines ~205-229.

## 4) Body/head/hair/beard specifics

### Body
- Body node graphic comes from body type naked path (or mutant/anomaly overrides, or dessicated path).
- `PawnRenderNodeWorker_Body` suppresses body for `NoBody`, some lying/bed cases, or duty overrides.
- Key files:
  - `Verse/PawnRenderNode_Body.cs` lines ~12-40.
  - `Verse/PawnRenderNodeWorker_Body.cs` lines ~8-35.

### Head
- Head node draws normal head type, skull when dessicated, none if no head.
- Head worker blocks draw when `HeadStump` flag set and applies head offset/narrow-head offsets.
- Key files:
  - `Verse/PawnRenderNode_Head.cs` lines ~22-33.
  - `Verse/PawnRenderNodeWorker_Head.cs` lines ~7-54.

### Hair
- Hair node uses `pawn.story.hairDef` and hair mesh set.
- Hidden for babies/newborns, or when skip flags suppress hair.
- Key file: `Verse/PawnRenderNode_Hair.cs` lines ~10-26.
- Hair graphic uses cutout hair shader by default:
  - `RimWorld/HairDef.cs`.

### Beard
- Beard node uses `pawn.style.beardDef`.
- Beard worker has extra facing behavior and head-type offset logic.
- Key files:
  - `Verse/PawnRenderNode_Beard.cs`.
  - `Verse/PawnRenderNodeWorker_Beard.cs` lines ~7-33.
  - `RimWorld/BeardDef.cs` for narrow-head offsets.

### Head stump
- Separate stump worker only draws when `HeadStump` flag is active.
- Key file: `Verse/PawnRenderNodeWorker_Stump.cs`.

## 5) Apparel layering and ordering

### Wear-time ordering source
- `Pawn_ApparelTracker` sorts worn apparel by `LastLayer.drawOrder`.
- This order is used when iterating `pawn.apparel.WornApparel` in dynamic node setup.
- Key file: `RimWorld/Pawn_ApparelTracker.cs` lines ~662-665.

### Layer defs
- `ApparelLayerDef.drawOrder` is the ordering primitive.
- Standard layers: `OnSkin`, `Middle`, `Shell`, `Belt`, `Overhead`, `EyeCover`.
- Key files:
  - `Verse/ApparelLayerDef.cs`.
  - `RimWorld/ApparelLayerDefOf.cs`.

### Dynamic apparel node construction
`DynamicPawnRenderNodeSetup_Apparel` does the main work:
- For each worn apparel item:
  - Optional custom render-node properties (`HasDefinedGraphicProperties`).
  - Fallback default node as head apparel or body apparel based on `LastLayer` and/or parent tag.
  - Per-parent incremental layer offsets (`layerOffsets`) stack items in iteration order.
- Special fallback drawData:
  - Shell on north forced to layer 88 (unless explicit drawData or shellRenderedBehindHead).
  - Utility-as-pack uses north 93 / south -3 defaults.
- Key file: `Verse/DynamicPawnRenderNodeSetup_Apparel.cs` lines ~13-134.

### Apparel graphic selection
- Uses body-type-specific texture suffix for most body clothes (`_BodyTypeName`), but not overhead/eye-cover/pack/placeholder paths.
- Shader path may switch to masked (`CutoutComplex`) depending on style/useWornGraphicMask.
- Key file: `RimWorld/ApparelGraphicRecordGetter.cs` lines ~20-34.

## 6) Hat/hair/beard/eyes suppression rules

`PawnRenderTree.AdjustParms(...)` mutates `skipFlags` before request generation:
- If apparel has explicit `renderSkipFlags`, those are applied.
- Else fallback rules:
  - `UpperHead` coverage => skip `Hair`.
  - `FullHead` coverage => skip `Hair`, `Beard`, `Eyes`.
- `forceEyesVisibleForRotations` can clear eye skip for specific facings.
- Gene state can force tattoo skip (`TattoosVisible == false`).
- Key file: `Verse/PawnRenderTree.cs` lines ~273-324.

Headgear visibility also depends on draw flags/portrait/bed visibility:
- `PawnRenderNodeWorker_Apparel_Head.HeadgearVisible(...)`.

## 7) Body-part-driven rendering behavior

### Missing parts affect apparel availability
- When hediff logic creates/updates missing parts, pawn apparel is revalidated.
- Apparel requiring now-missing groups is removed (`HasPartsToWear`).
- Key files:
  - `Verse/HediffSet.cs` lines ~341-367.
  - `RimWorld/Pawn_ApparelTracker.cs` lines ~627-641.
  - `RimWorld/ApparelUtility.cs` lines ~148-160.

### Hediff/body-part graphics
- Visible hediffs with render-node props become dynamic nodes with `node.bodyPart = h.Part`.
- Hediff worker supports body-part anchoring and mirrored-side flips.
- Key files:
  - `Verse/DynamicPawnRenderNodeSetup_Hediffs.cs` lines ~17-30.
  - `Verse/PawnRenderNodeWorker_Hediff.cs`.

### Anchor system
- Anchors are resolved from body type wound anchors by explicit tag or body-part group walk-up.
- Used for hediff overlays and eye placement.
- Key file: `RimWorld/PawnDrawUtility.cs`.

### Generic body-part checks in node worker
- `linkedBodyPartsGroup` on any node can hide visuals when no corresponding non-missing part exists.
- `bodyPart.visibleHediffRots` can constrain per-rotation visibility.
- Key file: `Verse/PawnRenderNodeWorker.cs` lines ~32-55.

## 8) Layer math and transforms

- Node layer value (`baseLayer` + rotational drawData layer + subworker tweaks) is converted by:
  - `PawnRenderUtility.AltitudeForLayer(layer)`.
- Layer clamp range is `[-10, 100]`, scaled to Y by `0.0003658537`.
- Key files:
  - `Verse/PawnRenderNodeWorker.cs` lines ~217-230.
  - `Verse/PawnRenderUtility.cs` lines ~253-256.

Transform stack per node:
- Offset/pivot/rotation/scale from each ancestor + node, then altitude translation.
- See `PawnRenderTree.TryGetMatrix(...)` and `ComputeMatrix(...)`.

## 9) Practical modding implications

- If you want deterministic apparel over/under behavior, control both:
  1. `ApparelLayerDef.drawOrder` / `LastLayer` (wear list order).
  2. Node `baseLayer`/`drawData` (actual render altitude).
- Prefer explicit `renderSkipFlags` on headgear to avoid fallback ambiguity.
- For body-part-specific visuals, use render-node props tied to hediff/gene/trait and body-part anchoring, plus `linkedBodyPartsGroup` guards.
- If visuals change at runtime (apparel, body part loss, gene/hediff changes), ensure render tree is dirtied (`SetAllGraphicsDirty` path) so requests rebuild.

## 10) Notes / limitations

- This repo appears to contain decompiled C# but not the XML def payloads for concrete `PawnRenderTreeDef` node hierarchies and exact base layer constants per race.
- So this writeup captures the rendering engine behavior and composition logic, but not the exact humanlike tree declaration data from defs.
