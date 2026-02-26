# Compatibility Profile v0 (Render)

This profile defines the minimum compatibility surface required to satisfy v0 render parity.

## Target Scope

- Play-map rendering (not world map rendering parity).
- Humanlike and non-humanlike pawn draw stacks.
- Def-driven graphics and overlays relevant to visible map output.

## Required Data/Def Support

## Thing/Terrain Rendering Fields

- `BuildableDef.altitudeLayer` and `BuildableDef.Altitude`.
- `ThingDef.drawerType`.
- `ThingDef.size`, `ThingDef.rotatable`.
- `ThingDef.graphicData` (core fields below).
- Terrain materials used by section layers (`DrawMatSingle`, water depth material path usage).

## `GraphicData` Fields (minimum)

- `texPath`
- `graphicClass`
- `shaderType`
- `color`, `colorTwo`
- `drawSize`
- `drawOffset` + per-rotation offsets
- `drawRotated`, `allowFlip`, `flipExtraRotation`
- `onGroundRandomRotateAngle`
- `linkType`/`linkFlags`
- `shadowData` (for shadow draw path)

## Pawn/Apparel Fields (minimum)

- Pawn body/head/hair graphic paths and colors.
- Apparel `wornGraphicPath`, `useWornGraphicMask`, `wornGraphicData` offsets/scales.
- Apparel layer semantics for shell/overhead/utility handling.

## Required Graphic Class Behaviors

- `Graphic_Single`
- `Graphic_Multi`
- `Graphic_Collection` subclasses (`Random`, `StackCount`, `Flicker`, `Appearances`, `Cluster` where used)
- Linked wrappers via `GraphicUtility.WrapLinked(...)`

## Required Draw Systems

- Section-map draw path (`MapDrawer` + `Section` + section layers).
- Dynamic draw path (`DynamicDrawManager`).
- Camera pre-cull weather draw path.
- Overlays: designations, meta overlays, selection overlays.
- Pawn renderer + pawn graphic set + equipment/overlay stack.

## Required Load/Precedence Semantics

- Active mod order execution.
- Patch application before inheritance resolve and def object creation.
- XML inheritance parent selection by load order.
- Def same-name replacement behavior.
- Content lookup precedence in reverse running mod order.

## Non-Goals In v0

- Full world-map render parity.
- Every edge-case patch operation behavior not affecting visible render outcomes.
- Every debug-only overlay.

## Compliance Status Labels

- `required`: must be implemented for v0 acceptance.
- `supported-later`: known gap intentionally deferred.
