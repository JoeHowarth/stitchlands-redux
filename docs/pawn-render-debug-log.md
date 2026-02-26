# Pawn Render Debug Log

## 2026-02-26 - Iteration 1

### Learnings
- Real-data pawn fixture can render many combinations, but several variants show floating or detached headgear/overlays.
- Current pipeline has enough variation to expose bad layering/offset interactions quickly.

### Hypotheses
- Head-anchored apparel is over-shifted vertically (anchor + extra offset stack may be too high).
- Missing/appx `drawData` handling for apparel is causing some hats and utility items to sit at invalid z/y.
- Variant selection was opaque; we need exact node traces per run to debug confidently.

### Actions
- Add trace dump support for composed pawn nodes (`--dump-pawn-trace`).
- Record selected body/head/hair/beard/apparel pieces in trace for each variant.

### Conclusions
- Need quantitative node traces before changing offsets again.

## 2026-02-26 - Iteration 2

### Learnings
- Directional texture suffixing (`_south/_north/_east`) is required for pawn layers; unsuffixed paths produce incorrect frames.
- Worn apparel data in defs is richer than our previous model: `drawData` layer overrides and `wornGraphicData` offset/scale materially affect final composition.
- Debug hediff overlays masked composition quality; clean validation requires overlays off by default.

### Hypotheses
- Remaining mismatch is mostly from incomplete parity with apparel render-node behavior (base layer math and mesh sizing), not random multiplier tuning.
- Explicit `renderSkipFlags` semantics matter: presence of explicit flags should override fallback body-part-based hide logic.

### Actions
- Parsed `wornGraphicPath` only for apparel fixtures and filtered out non-humanlike/non-worn entries.
- Added directional texture resolution with fallback to unsuffixed paths when directional variants are absent.
- Parsed and applied apparel `drawData` rotational layer overrides.
- Parsed and applied apparel `wornGraphicData` directional offset/scale data.
- Implemented explicit skip-flag handling path separate from fallback coverage rules.
- Switched fixture hediff overlays to opt-in via `STITCHLANDS_ENABLE_DEBUG_HEDIFFS`.

### Conclusions
- Pawn screenshots are materially closer to RimWorld and less artifact-prone.
- Core remaining gap is deeper parity with node base-layer hierarchy and mesh-size semantics.

## 2026-02-26 - Iteration 3

### Learnings
- RimWorld humanlike rendering uses mesh-set sizes around `1.5` for body/head and head-type mesh sizes for hair/beard.
- Keeping our pawn mesh basis at `1.0` produced compressed silhouettes and exaggerated head offset perception.

### Hypotheses
- Matching humanlike mesh sizing from decompiled `HumanlikeMeshPoolUtility` should improve overall proportions without ad-hoc offset multipliers.

### Actions
- Switched fixture body/head base size to `1.5`.
- Switched hair/beard size to parsed head-type mesh sizes directly.
- Scaled apparel draw size to the same mesh basis (`* 1.5`) before worn-graphic directional scaling.
- Re-ran deterministic 10-variant screenshot loop and trace dump.

### Conclusions
- Silhouette and proportion look closer to RimWorld baselines in south/east/west facings.
- Remaining fidelity work is primarily in exact node base-layer hierarchy and advanced render-tree node parenting.

## 2026-02-26 - Iteration 4

### Learnings
- `DynamicPawnRenderNodeSetup_Apparel` stacks layers per parent apparel root (`ApparelBody` vs `ApparelHead`), not with one global apparel index.
- Decompiled `PawnRenderNodeWorker.ScaleFor` multiplies node draw size with graphic draw size; for most apparel this effectively keeps baseline scale near `1.0` unless worn graphic data overrides it.
- Real fixture screenshots with `1.5` body/head/apparel basis were consistently oversized and less RimWorld-like.

### Hypotheses
- Parent-scoped apparel stacking plus `1.0` body/head/apparel basis will improve composition fidelity more than further y-offset tweaking.

### Actions
- Changed apparel stacking to maintain independent body/head stack indices.
- Set fixture body/head/apparel base sizes to `1.0`.
- Kept hair/beard sizing data-driven from head type defs (with `1.0` fallback).
- Re-ran deterministic 10-variant screenshot loop and regenerated cropped montage.

### Conclusions
- Silhouettes now track expected pawn proportions better, especially in mixed body+head gear combinations.
- Remaining mismatch appears concentrated in deeper render-tree parity details (node-parent behavior and some per-item drawData edge cases), not raw size multipliers.

## 2026-02-26 - Iteration 5

### Learnings
- We still had hardcoded layering assumptions in compose defaults even though real constants are declared in `Core/Defs/PawnRenderTreeDefs/PawnRenderTreeDefs.xml`.
- Visual debugging is easier when fixture combos avoid cross-body/head mismatches and random facings.

### Hypotheses
- Reading Humanlike render-tree layers directly from defs will make future tuning more stable and reduce hidden drift from decompiled assumptions.

### Actions
- Added `load_humanlike_render_tree_layers(...)` parser and tests in `src/defs.rs`.
- Wired fixture compose config to use parsed Humanlike layers for body/head/beard/hair/apparel z computation.
- Added fixture compatibility guard that prefers male heads for male bodies and female heads for female bodies.
- Forced `--pawn-fixture` renders to south-facing for deterministic visual comparison.
- Re-ran fmt/test/clippy and regenerated deterministic 10-variant screenshots/traces.

### Conclusions
- Composition is now backed by real render-tree layer data instead of hardcoded constants.
- Remaining visual deltas are likely in deeper worker behaviors (e.g. special-case node logic) rather than base layer ordering.

## 2026-02-26 - Iteration 6

### Learnings
- `ApparelProperties.parentTagDef` can override inferred head/body apparel anchoring and is used by dynamic node setup when present.

### Hypotheses
- Respecting `parentTagDef` in compose will avoid subtle mis-anchoring for apparel defs that intentionally attach outside default layer heuristics.

### Actions
- Parsed `parentTagDef` in apparel defs and normalized to tag token (`ApparelHead` / `ApparelBody`).
- Added `anchor_to_head` override in composed apparel input.
- Updated graph anchoring to prefer explicit anchor override before layer-based fallback.
- Re-ran fmt/test/clippy and deterministic 10-variant screenshot/trace loop.

### Conclusions
- Data model now supports explicit parent-tag anchoring parity.
- Remaining mismatches are no longer from missing core def fields; likely from worker-level behavior still simplified versus full RimWorld render tree.

## 2026-02-26 - Iteration 7

### Learnings
- We were still coupling Rim-local composition math with engine world/screen mapping inside compose node evaluation.
- Hypothesis test: flipping Rim `+z` to world `-y` in the new mapping produces globally upside-down pawns, so sign inversion is not the root cause.

### Hypotheses
- A principled boundary layer (`RimLocal -> World`) is required so future visual fixes happen in one adapter, not spread across workers and offset sites.

### Actions
- Added `RimToWorldTransform` to `PawnComposeConfig`.
- Changed compose evaluation to build Rim-local offsets first and apply one mapping call at the end.
- Added unit test proving transform sign behavior (`rim_z_to_world_y = -1` drives head below body).
- Ran controlled screenshot experiment with sign-flipped mapping and rejected it based on clear inversion artifacts.
- Restored default sign mapping and reran fmt/test/clippy.

### Conclusions
- Coordinate mapping is now explicit and testable, reducing risk of hidden band-aid fixes.
- Remaining fidelity work should target higher-level render-tree behavior parity, not coordinate sign hacks.
