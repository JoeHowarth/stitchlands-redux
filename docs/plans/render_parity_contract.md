# Render Parity Contract (v0)

This contract freezes what must match now for visual parity, based on decompiled source tracing.

## Must Match Now

## 1) Frame And Pass Order

- Match play frame entry flow: `Root_Play.Update` -> `Game.UpdatePlay` -> `Map.MapUpdate`.
- Match in-map draw pass order from `Map.MapUpdate`:
  - map mesh regen
  - map mesh draw
  - dynamic things
  - game condition draw
  - edge clippers
  - designations
  - overlays
  - temporary things
- Keep weather draw timing as camera pre-cull behavior, not as an inlined map pass.

## 2) Depth And Layering Rules

- Use `AltitudeLayer` + `AltitudeFor` spacing (`0.428571433` per layer) as base depth model.
- Preserve micro-offset conventions (`AltInc = 3/70`) where used for overlay stacking.
- Preserve map-mesh thing print behavior through section layers and thing true-center altitude.
- Preserve dynamic draw culling gates (view/fog/snow) before `thing.Draw()`.

## 3) Coordinate And Transform Model

- Cell center world transform must be `(x + 0.5, y + altitude, z + 0.5)`.
- Even-size thing center correction by rotation must match `GenThing.TrueCenter` behavior.
- World->UI conversion must follow camera screen transform and UI scale handling.

## 4) Graphic Resolution

- Match `GraphicData` resolution path into `GraphicDatabase` cache.
- Match texture naming conventions and fallback behavior for:
  - `Graphic_Single`
  - `Graphic_Multi`
  - `Graphic_Collection` family
  - `Graphic_Appearances`
- Match mask usage gate (`CutoutComplex` support path) and material request caching semantics.

## 5) Pawn Layering

- Match standing pawn render order (body, wounds, head, hair/overhead apparel, shell/utility, equipment, status overlays).
- Match laying/bed posture path and root altitude behavior.
- Match key Y-offset constants used to separate pawn sublayers.

## 6) Def/Patch/Content Precedence

- Match mod load/patch/inheritance/def-instantiation order.
- Match def override behavior (later same `defName` replaces earlier).
- Match content lookup precedence across active mods/resources/bundles.

## Later (Explicitly Deferred)

- Exact deterministic ordering of all reflected section-layer subclasses across arbitrary mod assemblies.
- Full shader-specific visual fidelity for every weather and mote variant.
- Non-map world rendering parity.
- Non-render gameplay semantic parity.

## Acceptance

v0 is accepted when all `parity_fixtures_v0.md` "must-match" assertions pass and no contract item above regresses.
