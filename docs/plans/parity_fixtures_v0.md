# Parity Fixtures v0

These fixtures are the minimum visual checks for v0 contract validation.

## Fixture Format

- `ID`: stable fixture id.
- `Scene`: minimal setup.
- `Must Match`: assertions required for v0.
- `Later`: checks intentionally deferred.

## F01: Base Terrain + Fog + Snow + Lighting

- ID: `F01_terrain_fog_snow_light`
- Scene: flat terrain patch with mixed fogged/unfogged cells, varied snow depth, roofed and unroofed cells, static blockers.
- Must Match:
  - terrain appears at `AltitudeLayer.Terrain`
  - fog alpha behavior around fog edges
  - snow alpha scales with snow depth
  - lighting overlay darkens roofed/blocked regions per glow logic
- Later:
  - pixel-perfect shader curve parity for all terrain variants.

## F02: Static Things In Map Mesh

- ID: `F02_mapmesh_things`
- Scene: several map-mesh things with different `altitudeLayer`, sizes, and rotations in same/adjacent cells.
- Must Match:
  - printed thing placement uses true center + per-rotation draw offset
  - altitude-layer separation is correct (including small offset increments)
  - multi-cell centering behavior matches rotation-dependent half-cell logic
- Later:
  - full coverage of every custom linked-graphic subtype.

## F03: Dynamic Draw Culling

- ID: `F03_dynamic_draw_culling`
- Scene: realtime-only things in/near viewport boundaries, fogged cells, and high snow cells.
- Must Match:
  - draw occurs only when pass culling allows (view/fog/snow rules)
  - `drawOffscreen` and `seeThroughFog` behavior is respected
- Later:
  - deterministic tie behavior for very large dynamic sets.

## F04: Weather And Camera Timing

- ID: `F04_weather_precull`
- Scene: active weather with visible world and screen overlays.
- Must Match:
  - weather overlays draw via camera pre-cull timing
  - weather appears above world content using weather altitude conventions
- Later:
  - all weather event variants and animation subtleties.

## F05: Designations And Meta Overlays

- ID: `F05_designation_meta_overlay`
- Scene: place designations plus forbidden/power/question overlays on mixed object sizes.
- Must Match:
  - designation draw pass appears after game-condition/clipper pass
  - overlay stack offsets and pulse layering are preserved
- Later:
  - exact pulse phase matching for every overlay class.

## F06: Pawn Standing Layer Stack

- ID: `F06_pawn_standing_layers`
- Scene: one humanlike pawn with apparel across shell/overhead/utility layers, primary weapon, damage state, and status overlay.
- Must Match:
  - draw order: body -> wounds -> head -> hair/overhead apparel -> shell/utility -> equipment -> status
  - facing-dependent utility/equipment offsets
  - hat-front-of-face and hair suppression rules
- Later:
  - every rare apparel combination across all mods.

## F07: Pawn Laying/Bed Stack

- ID: `F07_pawn_laying_bed`
- Scene: downed/dead pawns on ground and in bed with varying rotations.
- Must Match:
  - laying branch root altitude and angle/facing behavior
  - bed-driven root remap and render-body flags
- Later:
  - all race-specific corpse pose edge cases.

## F08: Graphic Path Fallbacks

- ID: `F08_graphic_fallbacks`
- Scene: assets with partial directional sets and masks (`_north` missing, etc.).
- Must Match:
  - multi-direction fallback/flip behavior matches decompiled rules
  - `_m` and directional mask loading rules are respected
- Later:
  - exotic shader parameter combinations not used in core fixtures.

## F09: Def/Patch Override Semantics (Render-Relevant)

- ID: `F09_def_patch_precedence`
- Scene: two mods overriding same render-relevant defs plus patch operations and inheritance.
- Must Match:
  - final resolved def for same `defName` matches load-order replacement semantics
  - patched XML contributes before inheritance/def instantiation
  - content lookup chooses last-loaded mod asset first
- Later:
  - non-render patch operation surface area.

## F10: End-To-End Golden Frame

- ID: `F10_golden_frame`
- Scene: one compact map containing terrain, static things, realtime things, one pawn, weather, overlays, and designations.
- Must Match:
  - one-frame ordered pass composition matches contract
  - relative depth/layer outcomes match expected golden output
- Later:
  - long-run animation/state drift checks.
