# Graphic Resolution Pipeline (Def -> Graphic -> Material -> Draw)

This document captures the render-relevant graphic resolution path from decompiled source.

## 1) Def And `GraphicData` Entry

1. Defs carry `graphicData` (`ThingDef`/`BuildableDef` path).
2. `GraphicData.Graphic` lazily initializes and caches via `Init()`.
3. `GraphicData.Init` picks shader (`shaderType` else `ShaderTypeDefOf.Cutout`) and calls `GraphicDatabase.Get(...)` with:
   - graphic class
   - texture path (`texPath`)
   - shader
   - draw size
   - color/colorTwo
   - optional shader parameters
4. Optional wrappers are then applied:
   - `Graphic_RandomRotated` if `onGroundRandomRotateAngle > 0`
   - linked wrapper via `GraphicUtility.WrapLinked(...)` if `linkType != None`

Source symbols:
- `Verse.GraphicData.Graphic` / `Init` (`Verse/GraphicData.cs`)
- `Verse.GraphicUtility.WrapLinked` (`Verse/GraphicUtility.cs`)

## 2) Graphic Cache Keying

- `GraphicDatabase` caches graphics by `GraphicRequest` key.
- Key fields include class, path, shader, draw size, color, colorTwo, graphicData ref, renderQueue, shaderParameters ref.
- If request not cached, `Init(req)` is called on the specific `Graphic` subclass.

Source symbols:
- `Verse.GraphicDatabase.Get` / `GetInner` (`Verse/GraphicDatabase.cs`)
- `Verse.GraphicRequest` (`Verse/GraphicRequest.cs`)

## 3) Texture Path Conventions By Graphic Type

## `Graphic_Single`
- Main texture: `path`
- Optional mask: `path + "_m"` when shader supports mask

Source: `Verse/Graphic_Single.cs`

## `Graphic_Multi`
- Directional textures:
  - `path + "_north"`
  - `path + "_east"`
  - `path + "_south"`
  - `path + "_west"`
- Directional masks (if mask-capable shader):
  - `path + "_northm"`, `..._eastm`, `..._southm`, `..._westm`
- Fallback behavior:
  - If north missing, falls back to south/east/west/base `path` in that order.
  - Missing east/west can be mirrored from opposite side with `allowFlip` rules.

Source: `Verse/Graphic_Multi.cs`

## `Graphic_Collection` Family (`Random`, `StackCount`, `Flicker`, etc.)
- Loads all textures in folder `path`.
- Excludes names ending with `_m`.
- Sorts by texture name.
- Creates subgraphics as `Graphic_Single` with `path + "/" + textureName`.

Source: `Verse/Graphic_Collection.cs`

## `Graphic_Appearances`
- Picks variant by `StuffAppearanceDef` and texture filename suffix matching appearance def name.
- Falls back to smooth appearance if no specific appearance match.

Source: `Verse/Graphic_Appearances.cs`

## 4) Variant Selection Rules

- `Graphic_Random`: picks subgraphic by `thing.thingIDNumber % count`.
- `Graphic_StackCount`: picks subgraphic by stack thresholds relative to `stackLimit`.
- `Graphic_Appearances`: picks by stuff appearance metadata.
- Pawn heads use specialized head database lookup by path/gender/crown/skin color.

Source symbols:
- `Verse.Graphic_Random.SubGraphicFor` (`Verse/Graphic_Random.cs`)
- `Verse.Graphic_StackCount.SubGraphicForStackCount` (`Verse/Graphic_StackCount.cs`)
- `Verse.Graphic_Appearances.SubGraphicFor` (`Verse/Graphic_Appearances.cs`)
- `Verse.GraphicDatabaseHeadRecords` (`Verse/GraphicDatabaseHeadRecords.cs`)

## 5) Material Resolution And Caching

- `MaterialPool.MatFrom(MaterialRequest)` caches by shader/mainTex/color/colorTwo/mask/renderQueue/shaderParameters.
- `MaterialRequest` equality includes shader params by reference.
- Color values are quantized to `Color32` before cache lookup.
- Mask usage is validated against shader capability.

Source symbols:
- `Verse.MaterialPool` (`Verse/MaterialPool.cs`)
- `Verse.MaterialRequest` (`Verse/MaterialRequest.cs`)

## 6) Mask/Shader Semantics

- `ShaderUtility.SupportsMaskTex(shader)` returns true only for `ShaderDatabase.CutoutComplex`.
- So `colorTwo` + mask channel behavior is effectively tied to CutoutComplex path.

Source symbol:
- `Verse.ShaderUtility.SupportsMaskTex` (`Verse/ShaderUtility.cs`)

## 7) Linked Graphic Atlas Behavior

- Linked graphics use `MaterialAtlasPool.SubMaterialFromAtlas(...)`.
- Atlas is a 4x4 UV slice set (16 link-direction variants) generated from a root material.

Source symbol:
- `Verse.MaterialAtlasPool` (`Verse/MaterialAtlasPool.cs`)

## 8) Content Precedence For Texture/Audio Lookup

- `ContentFinder<T>.Get(path)` search order:
  1. Active mods in reverse running order (last loaded checked first).
  2. Base resources (`Resources.Load`).
  3. Asset bundles.
- First hit wins.

Source symbol:
- `Verse.ContentFinder<T>.Get` (`Verse/ContentFinder.cs`)

## 9) Draw-Time Placement Inputs From Graphics

- `Graphic.DrawWorker` applies:
  - mesh from `MeshAt(rot)` (with rotation/flip logic)
  - positional offset from `GraphicData.DrawOffsetForRot(rot)`
  - optional extra rotation
- `Graphic.Print` uses `thing.TrueCenter() + DrawOffset(rot)` and prints to section mesh.

Source symbol:
- `Verse.Graphic` (`Verse/Graphic.cs`)

## 10) v0 Rules Snapshot

| Rule | Confidence | Needed For v0 |
|---|---|---|
| `GraphicData` -> `GraphicDatabase` cache resolution flow. | High | Yes |
| Single/multi texture suffix conventions and fallbacks. | High | Yes |
| Collection folder loading and deterministic name sort. | High | Yes |
| Material cache key semantics and mask support gate. | High | Yes |
| Linked atlas submaterial slicing behavior. | High | Yes |
| Content precedence across mods/resources/bundles. | High | Yes |
