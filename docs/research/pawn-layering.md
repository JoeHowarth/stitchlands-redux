# Pawn Layering And Offsets (Decompiled)

This document extracts pawn rendering behavior from decompiled symbols, including layer order, offsets, and facing/state differences.

## 1) Pawn Draw Entry

1. `Thing.Draw()` -> `Thing.DrawAt(DrawPos)`.
2. `Pawn.DrawPos` delegates to `Pawn_DrawTracker.DrawPos`.
3. `Pawn_DrawTracker.DrawPos` composes:
   - tweener position
   - jitter offset
   - lean offset
   - then sets `y = pawn.def.Altitude`.
4. `Pawn_DrawTracker.DrawAt` calls `PawnRenderer.RenderPawnAt`.

Source symbols:
- `Verse.Thing.Draw/DrawAt` (`Verse/Thing.cs`)
- `Verse.Pawn_DrawTracker.DrawPos/DrawAt` (`Verse/Pawn_DrawTracker.cs`)
- `Verse.PawnRenderer.RenderPawnAt` (`Verse/PawnRenderer.cs`)

## 2) Standing Pawn Render Stack

`PawnRenderer.RenderPawnInternal` executes standing pawn visuals in this order (humanlike path):

1. Body base materials (`PawnGraphicSet.MatsBodyBaseAt`).
2. Wound overlays (`PawnWoundDrawer`) on top of body.
3. Head mesh/material.
4. Overhead apparel and hair logic:
   - overhead apparel may draw before/after face depending on `hatRenderedFrontOfFace`.
   - hair can be suppressed by overhead apparel rules.
5. Shell and utility apparel extras (including pack-style utility rendering).
6. Equipment draw (`DrawEquipment`).
7. Worn extra visuals (`DrawWornExtras`).
8. Status overlays (`PawnHeadOverlays`).

Source symbols:
- `Verse.PawnRenderer.RenderPawnInternal` (`Verse/PawnRenderer.cs`)
- `Verse.PawnGraphicSet.MatsBodyBaseAt` (`Verse/PawnGraphicSet.cs`)

## 3) Canonical Y Offsets Used In Pawn Renderer

These constants are hard-coded in `PawnRenderer` and used to stack sublayers:

| Layer/Use | Offset |
|---|---|
| Body | `9/980` |
| Apparel step interval | `3/980` |
| Wounds | `9/490` |
| Shell region | `3/140` |
| Head region | `6/245` |
| Utility (general) | `27/980` |
| Utility (south-specific branch) | `3/490` |
| On-head layer | `3/98` |
| Post-head layer | `33/980` |
| Carried thing over/under | `+/- 9/245` |
| Primary equipment over | `9/245` |

Source symbol:
- `Verse.PawnRenderer` constants and draw branches (`Verse/PawnRenderer.cs`)

## 4) Facing Matrix (Standing)

| Facing | Head Offset Source | Utility Offset Branch | Idle Weapon Draw Rule |
|---|---|---|---|
| North | `BaseHeadOffsetAt(North)` | general utility branch (`27/980`) | Draw loc `(0, -0.11)` with base equipment Y path |
| South | `BaseHeadOffsetAt(South)` | south utility branch (`3/490`) | Draw loc `(0, -0.22)` and `+9/245` Y |
| East | `BaseHeadOffsetAt(East)` | general utility branch (`27/980`) | Draw loc `(+0.2, -0.22)` and `+9/245` Y |
| West | `BaseHeadOffsetAt(West)` | general utility branch (`27/980`) | Draw loc `(-0.2, -0.22)` and `+9/245` Y |

Source symbols:
- `Verse.PawnRenderer.BaseHeadOffsetAt` (`Verse/PawnRenderer.cs`)
- `Verse.PawnRenderer.DrawEquipment` (`Verse/PawnRenderer.cs`)

## 5) Laying/Downed/Bed Path

- Laying path uses `RenderPawnInternal` with `renderBody`/angle/facing derived from posture and bed context.
- In bed (humanlike), root loc is remapped to bed-based altitude/facing adjustment.
- If not in bed and not dead/carried, root Y is set to `AltitudeLayer.LayingPawn + 9/980`.

Source symbol:
- `Verse.PawnRenderer.RenderPawnAt` laying branch (`Verse/PawnRenderer.cs`)

## 6) Pawn Graphic Resolution

- `PawnGraphicSet.ResolveAllGraphics` resolves:
  - body graphics (fresh/rotting/dessicated)
  - head/skull/stump graphics
  - hair graphic
  - apparel graphics
- Humanlike bodies use explicit body/head/hair databases; non-humanlike path uses life-stage graphic data.

Source symbol:
- `Verse.PawnGraphicSet.ResolveAllGraphics` (`Verse/PawnGraphicSet.cs`)

## 7) Apparel Graphic Path And Offsets

- `ApparelGraphicRecordGetter.TryGetGraphicApparel` path convention:
  - Usually `wornGraphicPath + "_" + bodyType.defName`
  - Overhead/pack/placeholder keep base `wornGraphicPath`.
- Utility/belt-like rendering can use `WornGraphicData.BeltOffsetAt` and `BeltScaleAt` by facing/body type.

Source symbols:
- `RimWorld.ApparelGraphicRecordGetter` (`RimWorld/ApparelGraphicRecordGetter.cs`)
- `RimWorld.WornGraphicData` (`RimWorld/WornGraphicData.cs`)

## 8) v0 Confidence Snapshot

| Rule | Confidence | Needed For v0 |
|---|---|---|
| Standing layer order and major branch logic. | High | Yes |
| Core Y offsets used for stack separation. | High | Yes |
| Facing-specific equipment draw placement. | High | Yes |
| Bed/laying branch altitudes. | High | Yes |
| Apparel path suffix/body-type rules. | High | Yes |
