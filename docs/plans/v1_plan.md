# v1 Plan

## Goal

- Render a small playable scene with:
  - RimWorld terrain tiles (real assets)
  - placed things
  - simple pawns
- Keep parity "good enough" for composition/order, not full fidelity yet.

## Scope for v1

- Include: terrain map, things, pawn baseline, camera movement, deterministic sorting.
- Exclude: weather/overlays/designations (v2+), full pawn apparel stack, full map section system parity.

## Steps

1. Terrain asset validation pass

- Add a quick probe command to answer:
  - which terrain-related textures decode successfully with current typetree registry
  - which fail
- Output: top candidate terrain texture names + decode status.
- Exit criteria: at least 2-3 terrain texture families confirmed decodable.

2. TerrainDef support (render-relevant subset)

- Extend def parsing to include minimal `TerrainDef` fields needed for visuals:
  - base texture/material path hooks used by terrain graphics
  - optional edge/overlay paths if easy
- Reuse existing texture resolver path (loose -> packed -> fallback).
- Exit criteria: terrain defs resolve into drawable tile sprite descriptors.

3. Map data model + fixture scene

- Introduce a minimal map representation:
  - width/height grid
  - terrain per cell
  - thing instances
  - pawn instances
- Start with generated fixture map (not save-file parsing yet):
  - e.g. 40x40 with 3 terrain types in patches
- Exit criteria: generated map can be rendered deterministically from one command.

4. Renderer upgrade for tilemap + world instances

- Add tile draw pass before things/pawns.
- Add batching/instancing for tile sprites (needed for perf even in v1).
- Keep deterministic draw ordering:
  - terrain first, then things, then pawns
  - stable tie-breaks
- Exit criteria: camera pan/zoom over full tile grid remains smooth and stable.

5. Things on map

- Reuse current `ThingDef` path, but place many instances on cells.
- Keep single-pass thing rendering with basic altitude sorting.
- Exit criteria: multiple things mixed over terrain, no fallback for known-good defs.

6. Pawn baseline

- Implement intentionally simple pawn draw first:
  - single pawn texture or minimal body+head stack
  - facing (N/E/S/W) if assets are available
- No apparel complexity in v1.
- Exit criteria: several pawns render correctly above terrain/things and move layer-wise as expected.

7. v1 smoke harness

- Add integration smoke scene command + screenshot output:
  - asserts no fallback for selected terrain + thing + pawn assets
  - stores a deterministic screenshot for quick regressions
- Exit criteria: one repeatable command validates v1 scene end-to-end.

## Acceptance

- One command launches a map scene with real RimWorld terrain tiles, things, and pawns.
- Scene uses real assets (not checker fallback) for chosen v1 fixture assets.
- Sorting/composition is stable and believable.
- Automated smoke check exists for this path.

## Primary Risk

- Terrain and pawn assets may have different decode reliability than `Steel`; lock a known-good asset set early and build v1 fixture around those first.

## Pending Decision

- For v1 map source:
  1. Generated fixture map (recommended first)
  2. Parse a real RimWorld save/map file now
- Option 1 gets to visible progress much faster.
