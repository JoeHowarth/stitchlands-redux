# v2 Plan

## Goal

- Make the map feel interactive and alive through movement and player feedback.
- Prioritize believable pawn motion and first interactions over visual effects.

## v2 Focus Areas

- Moving pawns look correct and readable while navigating the map.
- First interaction loop exists: selection, hover feedback, and click-to-move path intent.
- Scene/fixture setup shifts from large Rust builders toward data-driven fixture files.

## Scope

- Include:
  - Pawn movement and stable dynamic ordering.
  - Interaction overlays: selection, hover, path/job intent lines.
  - Data-driven micro-scene fixtures using RON.
  - Def-name based asset references in fixture data.
- Exclude:
  - Weather and atmospheric passes.
  - Full task/job simulation systems.
  - Full save/map parsing.

## Fixture Direction

- Use small deterministic micro-scenes as the primary fixture style.
- Keep fixture content data-driven (scene layout and placed entities), while behavior remains code-driven for now.
- Use RON as the fixture source format.

## Validation Strategy

- Prefer closed-loop iteration that an agent can run autonomously:
  - deterministic fixture runs,
  - validation commands,
  - reproducible screenshots/logs.
- Keep screenshot checks tolerant during active visual iteration.
- Emphasize structural assertions (ordering, selection state, path visibility, entity counts) over strict pixel parity.

## Success Criteria

- Pawn movement in fixture scenes is visually stable and believable.
- Selection and hover feedback are clear and responsive.
- Click-to-move displays intent/path feedback reliably.
- Key fixtures are loaded from RON files rather than hardcoded Rust scene construction.

## Deferred to Later Versions

- Weather, fog, and other atmospheric systems.
- Broader overlay/effects stack beyond interaction-critical overlays.
- Task/job authoring and simulation depth.
