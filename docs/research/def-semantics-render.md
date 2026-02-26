# Def/Patch Semantics That Affect Rendering

This file captures only the data-loading semantics that materially affect final render output.

## 1) High-Level Load Sequence

`PlayDataLoader.DoPlayLoad` runs this relevant order:

1. Clear graphics cache.
2. Load all active mods (`LoadedModManager.LoadAllActiveMods`).
3. Copy defs from mods into global def databases.
4. Resolve cross refs and implied defs.
5. Resolve all def references.

Source symbol:
- `Verse.PlayDataLoader.DoPlayLoad` (`Verse/PlayDataLoader.cs`)

## 2) Mod XML Pipeline Ordering

`LoadedModManager.LoadAllActiveMods` performs:

1. Initialize active mods (`runningMods` in active load order).
2. Load mod content (textures/audio/strings/assemblies/bundles).
3. Load def XML assets from all running mods.
4. Combine all XML nodes into one unified `<Defs>` document.
5. Apply patch operations to unified XML.
6. Register XML inheritance links, resolve inheritance, then instantiate defs.

Source symbol:
- `Verse.LoadedModManager.LoadAllActiveMods` (`Verse/LoadedModManager.cs`)

## 3) XML File Override Semantics Within A Mod

- `ModContentPack.GetAllFilesForMod` gathers files across mod load folders and keeps the first seen relative path.
- Because folders are traversed in `foldersToLoadDescendingOrder`, earlier folders in that list have precedence for duplicate relative files.

Source symbol:
- `Verse.ModContentPack.GetAllFilesForMod` (`Verse/ModContentPack.cs`)

## 4) XML Inheritance Semantics (`Name`/`ParentName`)

- Nodes with `Name` and/or `ParentName` are registered for inheritance processing.
- Parent selection is load-order aware:
  - For mod nodes, choose best parent with load order `<=` current mod, preferring nearest higher eligible load order.
  - Fallback to non-mod node when needed.
- `Inherit="false"` clears inherited child nodes and uses child contents directly.
- Duplicate non-list child node names are treated as XML errors.

Source symbol:
- `Verse.XmlInheritance` (`Verse/XmlInheritance.cs`)

## 5) Patch Application Timing

- Patch operations run against the unified XML document before inheritance resolution and before def object instantiation.
- This means patched XML participates in subsequent inheritance and def creation.

Source symbol:
- `Verse.LoadedModManager.ApplyPatches` (`Verse/LoadedModManager.cs`)

## 6) Def Name Conflict Semantics

`DefDatabase<T>.AddAllInMods` behavior:

- Iterates running mods ordered by `OverwritePriority`, then by running list index.
- For each new def:
  - if same `defName` already exists, previous one is removed and new one is added.
  - net behavior is later source wins for shared `defName`.
- Patched defs (`LoadedModManager.PatchedDefsForReading`) are added after mod defs and can replace prior entries.

Source symbol:
- `Verse.DefDatabase<T>.AddAllInMods` (`Verse/DefDatabase.cs`)

## 7) Content Asset Lookup Precedence (Texture/Audio)

- `ContentFinder<T>.Get(path)` checks active mods in reverse running order (last loaded first), then base resources, then asset bundles.
- First match wins.

Source symbol:
- `Verse.ContentFinder<T>.Get` (`Verse/ContentFinder.cs`)

## 8) Render-Relevant Fields That Depend On Final Resolved Defs

These fields must be interpreted after all patch/inheritance/override rules above:

- `ThingDef.graphicData` (`texPath`, `graphicClass`, `shaderType`, colors, draw offsets/sizes).
- `BuildableDef.altitudeLayer` / `BuildableDef.Altitude`.
- `ThingDef.drawerType` (`MapMeshOnly`, `RealtimeOnly`, etc.).
- Pawn/apparel graphic paths and masks.
- Terrain/material references used by section layers.

## 9) Minimum Viable Compatibility For Render v0

| Semantic | Required For v0 | Confidence |
|---|---|---|
| Active mod load order preserved. | Yes | High |
| XML patch-before-inheritance-before-def-instantiation flow. | Yes | High |
| ParentName inheritance parent selection by load order. | Yes | High |
| Same-def-name replacement (later source wins). | Yes | High |
| Reverse-order content lookup across active mods. | Yes | High |

## 10) Known Deferred Areas

- Full patch operation surface behavior details beyond render-relevant outcomes.
- Non-render gameplay def semantics.
