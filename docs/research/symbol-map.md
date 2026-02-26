# Render Research Symbol Map

This symbol map captures the highest-value decompiled symbols for render parity work. It is organized by reading priority so we can trace one frame quickly without file-by-file spelunking.

## Priority Legend

- P0: Required to reconstruct one map frame.
- P1: Required for visual parity behavior (graphics, pawns, overlays).
- P2: Supporting semantics and compatibility details.

## P0: Frame Entry And Core Draw Stack

| Priority | Symbol | Decompiled Path | Why Read |
|---|---|---|---|
| P0 | `Verse.Root_Play.Update` | `Verse/Root_Play.cs` | Play-scene frame entry; calls `Current.Game.UpdatePlay()`. |
| P0 | `Verse.Game.UpdatePlay` | `Verse/Game.cs` | Calls `Map.MapUpdate()` for each map each frame. |
| P0 | `Verse.Map.MapUpdate` | `Verse/Map.cs` | Main map draw stack order and draw system orchestration. |
| P0 | `Verse.MapDrawer.MapMeshDrawerUpdate_First` | `Verse/MapDrawer.cs` | Section mesh regeneration scheduling. |
| P0 | `Verse.MapDrawer.DrawMapMesh` | `Verse/MapDrawer.cs` | Draws visible section layers. |
| P0 | `Verse.DynamicDrawManager.DrawDynamicThings` | `Verse/DynamicDrawManager.cs` | Realtime thing draw pass and culling rules. |

## P1: Section Layers, Depth, And Overlays

| Priority | Symbol | Decompiled Path | Why Read |
|---|---|---|---|
| P1 | `Verse.Section..ctor` | `Verse/Section.cs` | Section layer discovery/instantiation via reflection. |
| P1 | `Verse.Section.DrawSection` | `Verse/Section.cs` | Effective per-section layer draw iteration. |
| P1 | `Verse.SectionLayer.DrawLayer` | `Verse/SectionLayer.cs` | Submesh draw behavior and material batching entry. |
| P1 | `Verse.SectionLayer_Things.Regenerate` | `Verse/SectionLayer_Things.cs` | Printed-thing inclusion rules in map mesh. |
| P1 | `Verse.SectionLayer_Terrain.Regenerate` | `Verse/SectionLayer_Terrain.cs` | Terrain mesh generation and blending edges. |
| P1 | `Verse.SectionLayer_FogOfWar.Regenerate` | `Verse/SectionLayer_FogOfWar.cs` | Fog mesh generation and alpha logic. |
| P1 | `Verse.SectionLayer_Snow.Regenerate` | `Verse/SectionLayer_Snow.cs` | Snow overlay generation and depth-to-alpha behavior. |
| P1 | `Verse.SectionLayer_LightingOverlay.Regenerate` | `Verse/SectionLayer_LightingOverlay.cs` | Lighting overlay mesh/colors and roof minimum sky cover behavior. |
| P1 | `RimWorld.OverlayDrawer.DrawAllOverlays` | `RimWorld/OverlayDrawer.cs` | Forbidden/power/question mark overlay stack and offsets. |
| P1 | `Verse.DesignationManager.DrawDesignations` | `Verse/DesignationManager.cs` | Designation draw pass location in frame. |
| P1 | `Verse.MapEdgeClipDrawer.DrawClippers` | `Verse/MapEdgeClipDrawer.cs` | World clipper pass placement/altitude. |
| P1 | `Verse.TemporaryThingDrawer.Draw` | `Verse/TemporaryThingDrawer.cs` | Final temporary draw pass behavior. |

## P1: Weather, Camera, And World/UI Overlay Path

| Priority | Symbol | Decompiled Path | Why Read |
|---|---|---|---|
| P1 | `Verse.CameraDriver.OnPreCull` | `Verse/CameraDriver.cs` | Weather draw timing (outside map draw stack). |
| P1 | `RimWorld.WeatherManager.DrawAllWeather` | `RimWorld/WeatherManager.cs` | Calls both weather workers and weather events. |
| P1 | `Verse.WeatherWorker.DrawWeather` | `Verse/WeatherWorker.cs` | Weather overlay list draw dispatch. |
| P1 | `Verse.SkyOverlay.DrawOverlay` | `Verse/SkyOverlay.cs` | World/screen weather overlay drawing and altitude use. |
| P1 | `RimWorld.UIRoot_Play.UIRootOnGUI` | `RimWorld/UIRoot_Play.cs` | GUI composition and map UI hook points. |
| P1 | `RimWorld.MapInterface.MapInterfaceOnGUI_*` | `RimWorld/MapInterface.cs` | World-space labels, readouts, and map GUI overlay order. |
| P1 | `RimWorld.MapInterface.MapInterfaceUpdate` | `RimWorld/MapInterface.cs` | Selection/room overlay update-side drawing hooks. |

## P1: Pawn Rendering Stack

| Priority | Symbol | Decompiled Path | Why Read |
|---|---|---|---|
| P1 | `Verse.Pawn_DrawTracker.DrawPos` | `Verse/Pawn_DrawTracker.cs` | Pawn root draw position composition (tween/jitter/lean + altitude). |
| P1 | `Verse.PawnRenderer.RenderPawnAt` | `Verse/PawnRenderer.cs` | Standing vs laying branch and carried-thing handling. |
| P1 | `Verse.PawnRenderer.RenderPawnInternal` | `Verse/PawnRenderer.cs` | Exact body/head/hair/apparel/equipment/status draw order and offsets. |
| P1 | `Verse.PawnGraphicSet.ResolveAllGraphics` | `Verse/PawnGraphicSet.cs` | Pawn body/head/hair/apparel graphic resolution path. |
| P1 | `RimWorld.ApparelGraphicRecordGetter.TryGetGraphicApparel` | `RimWorld/ApparelGraphicRecordGetter.cs` | Apparel graphic path conventions and mask shader selection. |
| P1 | `RimWorld.WornGraphicData.BeltOffsetAt/BeltScaleAt` | `RimWorld/WornGraphicData.cs` | Utility apparel per-facing/per-body-type offsets/scales. |

## P1: Graphic Resolution And Material Caching

| Priority | Symbol | Decompiled Path | Why Read |
|---|---|---|---|
| P1 | `Verse.GraphicData.Init` | `Verse/GraphicData.cs` | Def -> graphic class/shader/path resolution and wrappers. |
| P1 | `Verse.GraphicDatabase.Get` | `Verse/GraphicDatabase.cs` | Graphic cache keying and class dispatch. |
| P1 | `Verse.Graphic_Single.Init` | `Verse/Graphic_Single.cs` | Single texture + optional `_m` mask loading convention. |
| P1 | `Verse.Graphic_Multi.Init` | `Verse/Graphic_Multi.cs` | `_north/_east/_south/_west` path rules + fallback/flip behavior. |
| P1 | `Verse.Graphic_Collection.Init` | `Verse/Graphic_Collection.cs` | Folder-based variant loading and deterministic sort by texture name. |
| P1 | `Verse.MaterialPool.MatFrom` | `Verse/MaterialPool.cs` | Material request caching and shader parameter application. |
| P1 | `Verse.MaterialAtlasPool.SubMaterialFromAtlas` | `Verse/MaterialAtlasPool.cs` | Link-direction atlas slicing for linked graphics. |
| P1 | `Verse.ShaderUtility.SupportsMaskTex` | `Verse/ShaderUtility.cs` | Mask support gate (`CutoutComplex` only). |

## P2: Def Loading, XML Inheritance, Mod Precedence

| Priority | Symbol | Decompiled Path | Why Read |
|---|---|---|---|
| P2 | `Verse.PlayDataLoader.LoadAllPlayData/DoPlayLoad` | `Verse/PlayDataLoader.cs` | High-level data load phases before gameplay rendering. |
| P2 | `Verse.LoadedModManager.LoadAllActiveMods` | `Verse/LoadedModManager.cs` | Mod load pipeline: XML combine, patch, inheritance resolve, def creation. |
| P2 | `Verse.LoadedModManager.ApplyPatches` | `Verse/LoadedModManager.cs` | Patch application timing relative to inheritance/def instantiation. |
| P2 | `Verse.XmlInheritance.Resolve` | `Verse/XmlInheritance.cs` | Parent selection and inherited node merge semantics. |
| P2 | `Verse.DefDatabase<T>.AddAllInMods` | `Verse/DefDatabase.cs` | Same-def-name replacement behavior and final precedence. |
| P2 | `Verse.ContentFinder<T>.Get` | `Verse/ContentFinder.cs` | Texture/audio content precedence across active mods and resources. |
| P2 | `Verse.ModContentPack.GetAllFilesForMod` | `Verse/ModContentPack.cs` | Within-mod file override semantics across load folders. |

## Read Order Recommendation

1. `Root_Play.Update` -> `Game.UpdatePlay` -> `Map.MapUpdate`.
2. `MapDrawer` + `Section` + key `SectionLayer_*` classes.
3. `DynamicDrawManager` and weather (`CameraDriver.OnPreCull`, `WeatherManager`).
4. `PawnRenderer` + `PawnGraphicSet` + apparel helpers.
5. `GraphicData` + `GraphicDatabase` + `Graphic_*` implementations.
6. `LoadedModManager` + `XmlInheritance` + `DefDatabase` + `ContentFinder`.
