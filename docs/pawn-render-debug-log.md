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
