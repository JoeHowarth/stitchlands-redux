Let's do this like a focused reverse-engineering pass with deliverables, not a giant spelunk.

## What We Need To Learn (from decompiled source)

- **Render pass order**
  - Exact high-level draw sequence (terrain, things, pawns, weather, fog, roofs, overlays, UI/world-space labels).
  - Which systems can inject into the stack and in what order.

- **Depth/sorting rules**
  - How draw priority is computed (altitude layers, drawPos axis, ties, stable ordering).
  - Special cases (items on tables, plant/tree overlays, filth, blueprints/frames).

- **Coordinate + transform model**
  - Cell -> world -> screen transform rules.
  - Camera zoom behavior and pixel snapping behavior.
  - Which axis controls "in front/behind" in sorting.

- **Sprite placement conventions**
  - Pivot/origin assumptions by graphic type.
  - `drawSize`, `drawOffset`, rotation offsets, altitude offsets.
  - Multi-part pawn rendering: body/head/hair/apparel layering and offsets by facing.

- **Graphic resolution pipeline**
  - How `GraphicData` and `Graphic*` classes resolve texture paths.
  - Atlas behavior / material caching / variant selection.
  - Masking/tinting/shader hints used by common assets.

- **Def resolution semantics (render-relevant only)**
  - XML inheritance and patch/load order rules that affect final render output.
  - Mod content precedence and fallback logic.

- **Overlays/effects**
  - Fog/roof/snow/weather rendering rules.
  - Selection brackets, mouseover highlights, designation overlays.

- **Parity-critical constants**
  - Altitude enum values, layer constants, magic offsets, default sizes.

## How We'll Go Find It

- **Step 1: Build a target symbol map**
  - Identify likely classes/methods first (renderer entry points, graphics classes, camera/map drawers, pawn renderer, overlay drawers).
  - Output: `docs/research/symbol-map.md` with "where to read first".

- **Step 2: Trace one frame end-to-end**
  - Start from top-level map/world draw method, follow calls in order.
  - Record pass order + sort key composition.
  - Output: `docs/research/frame-pipeline.md` (ordered bullets, no speculation).

- **Step 3: Deep dive into graphic resolution**
  - Trace from def -> graphic object -> material/texture path -> draw call.
  - Capture path conventions, fallback, rotation/variant behavior.
  - Output: `docs/research/graphic-resolution.md`.

- **Step 4: Pawn renderer extraction**
  - Trace full pawn draw stack by facing and state.
  - Capture layer order and offsets.
  - Output: `docs/research/pawn-layering.md` with a small matrix/table.

- **Step 5: Def/patch/minimal mod semantics**
  - Only parse what impacts renderer outputs.
  - Output: `docs/research/def-semantics-render.md` (minimum viable compatibility).

- **Step 6: Convert research into implementation contract**
  - Freeze decisions into:
    - `render_parity_contract.md`
    - `compat_profile_v0.md`
    - test fixtures list for parity checks.
  - Output includes "must-match now" vs "later".

## Execution Tactics (to stay efficient)

- Timebox each area (e.g., 60–90 min) and log unknowns instead of rabbit-holing.
- Prefer call-chain tracing over reading entire files.
- For each discovered rule, record:
  - source symbol
  - rule in plain language
  - confidence (high/medium/low)
  - whether needed for v0.
- Build tiny parity fixtures early (e.g., one terrain tile, one thing, one pawn with apparel) to validate assumptions immediately.

## Definition of Done for Research Phase

- We can answer, with references:
  - exact draw pass order
  - exact sort key inputs
  - graphic path/material resolution rules
  - pawn layering rules
  - minimal def semantics required for visual parity v0
