# Frame Pipeline (Decompiled Trace)

This file traces one play-scene frame end-to-end from decompiled source, with ordered calls only.

## 1) Play Frame Entry

1. `Verse.Root_Play.Update` runs play-scene update and calls `Current.Game.UpdatePlay()`.
2. `Verse.Game.UpdatePlay` runs game systems, then loops maps and calls `maps[i].MapUpdate()`.

Source symbols:
- `Verse.Root_Play.Update` (`Verse/Root_Play.cs`)
- `Verse.Game.UpdatePlay` (`Verse/Game.cs`)

## 2) Map Draw Stack In `Map.MapUpdate`

The map-side call order inside `Verse.Map.MapUpdate` is:

1. `skyManager.SkyManagerUpdate()`
2. `powerNetManager.UpdatePowerNetsAndConnections_First()`
3. `regionGrid.UpdateClean()`
4. `regionAndRoomUpdater.TryRebuildDirtyRegionsAndRooms()`
5. `glowGrid.GlowGridUpdate_First()`
6. `lordManager.LordManagerUpdate()`
7. If current map is rendered now (`!WorldRenderedNow && Find.CurrentMap == this`):
   1. Optional full shadow redraw dirty (`AlwaysRedrawShadows`)
   2. `PlantFallColors.SetFallShaderGlobals(this)`
   3. `waterInfo.SetTextures()`
   4. `avoidGrid.DebugDrawOnMap()`
   5. `mapDrawer.MapMeshDrawerUpdate_First()`
   6. `powerNetGrid.DrawDebugPowerNetGrid()`
   7. `DoorsDebugDrawer.DrawDebug()`
   8. `mapDrawer.DrawMapMesh()`
   9. `dynamicDrawManager.DrawDynamicThings()`
   10. `gameConditionManager.GameConditionManagerDraw(this)`
   11. `MapEdgeClipDrawer.DrawClippers(this)`
   12. `designationManager.DrawDesignations()`
   13. `overlayDrawer.DrawAllOverlays()`
   14. `temporaryThingDrawer.Draw()`
8. `areaManager.AreaManagerUpdate()`
9. `weatherManager.WeatherManagerUpdate()`
10. `MapComponentUtility.MapComponentUpdate(this)`

Source symbol:
- `Verse.Map.MapUpdate` (`Verse/Map.cs`)

## 3) Mesh Sub-Pipeline (`MapDrawer` -> `Section` -> `SectionLayer`)

1. `MapMeshDrawerUpdate_First` tries to regenerate dirty visible sections first; if none changed, it scans all sections until one change is processed.
2. `DrawMapMesh` iterates visible sections and calls `Section.DrawSection(...)`.
3. `Section.DrawSection` iterates `layers` in list order and calls each layer's `DrawLayer()`.
4. `Section` layer list is built by enumerating `typeof(SectionLayer).AllSubclassesNonAbstract()`.
5. `AllSubclassesNonAbstract` iterates `GenTypes.AllTypes` (executing assembly first, then active mod assemblies), then filters by subclass/non-abstract.

Source symbols:
- `Verse.MapDrawer.MapMeshDrawerUpdate_First` (`Verse/MapDrawer.cs`)
- `Verse.MapDrawer.DrawMapMesh` (`Verse/MapDrawer.cs`)
- `Verse.Section.DrawSection` (`Verse/Section.cs`)
- `Verse.Section..ctor` (`Verse/Section.cs`)
- `Verse.GenTypes.AllTypes` / `AllSubclassesNonAbstract` (`Verse/GenTypes.cs`)

## 4) Dynamic Draw Pass Rules

`DynamicDrawManager.DrawDynamicThings` draws from `drawThings` with these gates:

- In current view rect or `def.drawOffscreen`.
- Not fogged unless `def.seeThroughFog`.
- Not hidden by snow beyond `def.hideAtSnowDepth`.

Each passing thing calls `thing.Draw()` directly in this pass.

Source symbol:
- `Verse.DynamicDrawManager.DrawDynamicThings` (`Verse/DynamicDrawManager.cs`)

## 5) Weather And Overlay Injection Points

- Weather visual draw is triggered by camera, not by `Map.MapUpdate`:
  - `CameraDriver.OnPreCull` -> `Find.CurrentMap.weatherManager.DrawAllWeather()`.
- Weather manager draw sequence:
  - `eventHandler.WeatherEventsDraw()`
  - `lastWeather.Worker.DrawWeather(map)`
  - `curWeather.Worker.DrawWeather(map)`
- Weather worker draws each `SkyOverlay` via `DrawOverlay(map)`.

Source symbols:
- `Verse.CameraDriver.OnPreCull` (`Verse/CameraDriver.cs`)
- `RimWorld.WeatherManager.DrawAllWeather` (`RimWorld/WeatherManager.cs`)
- `Verse.WeatherWorker.DrawWeather` (`Verse/WeatherWorker.cs`)
- `Verse.SkyOverlay.DrawOverlay` (`Verse/SkyOverlay.cs`)

## 6) GUI And World-Space Label Path (Same Frame)

- `UIRoot_Play.UIRootUpdate` runs `mapUI.MapInterfaceUpdate()` each frame.
- `MapInterfaceUpdate` includes:
  - `SelectionDrawer.DrawSelectionOverlays()`
  - `EnvironmentStatsDrawer.DrawRoomOverlays()`
  - map debug overlay calls.
- `UIRoot_Play.UIRootOnGUI` runs map GUI passes:
  - `MapInterfaceOnGUI_BeforeMainTabs()` (thing overlays, tooltips, readouts)
  - main tabs/alerts
  - `MapInterfaceOnGUI_AfterMainTabs()` (environment stats GUI, deep resources GUI, debug GUI)

Source symbols:
- `RimWorld.UIRoot_Play.UIRootUpdate` (`RimWorld/UIRoot_Play.cs`)
- `RimWorld.MapInterface.MapInterfaceUpdate` (`RimWorld/MapInterface.cs`)
- `RimWorld.UIRoot_Play.UIRootOnGUI` (`RimWorld/UIRoot_Play.cs`)
- `RimWorld.MapInterface.MapInterfaceOnGUI_BeforeMainTabs` (`RimWorld/MapInterface.cs`)
- `RimWorld.MapInterface.MapInterfaceOnGUI_AfterMainTabs` (`RimWorld/MapInterface.cs`)

## 7) Sort/Depth Inputs Confirmed So Far

| Rule | Evidence | Confidence | Needed For v0 |
|---|---|---|---|
| Pass ordering is explicit in `Map.MapUpdate` and must be matched. | Direct ordered calls in method body. | High | Yes |
| Map-mesh printed things depend on section layer order + per-thing altitude in printed vertices. | `Section.DrawSection`, `SectionLayer_Things`, `Thing.Print`, `Thing.TrueCenter`. | High | Yes |
| Dynamic draw pass has no explicit global sort call; draw container is `HashSet<Thing>`. | `DynamicDrawManager.drawThings` + foreach iteration. | Medium | Yes |
| Weather overlays are camera pre-cull draw, outside `Map.MapUpdate` pass body. | `CameraDriver.OnPreCull`. | High | Yes |
| GUI/world-space labels are split between update-side draw calls and OnGUI calls. | `UIRoot_Play` + `MapInterface`. | High | Yes |

## 8) Unknowns Logged

- Final deterministic section-layer ordering across all loaded assemblies is not explicitly sorted in code (depends on type enumeration order).
- Some special-case visual behaviors (certain motes/shaders/subcamera effects) still need fixture-based verification.
