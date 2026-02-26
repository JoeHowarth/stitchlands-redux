You are taking over a focused technical investigation for a Rust `wgpu` prototype that aims to render RimWorld-compatible assets.

Context:
- Repo: `stitchlands-redux`
- Platform: macOS, Steam RimWorld install layout
- Current state:
  - Def loading works (`ThingDef` + `graphicData`)
  - Renderer works (real image rendering via `--image-path` confirmed)
  - Loose file lookup for many `ThingDef` textures fails because install has no loose `Core/Textures/*.png`
  - Packed Unity texture scan finds thousands of `Texture2D` objects and names (e.g. `steel_a`, `steel_b`, `steel_c`)
  - Packed decode currently fails for tested candidates with `Invalid data: Invalid texture dimensions`
  - Upstream `unity-asset-decode` example also fails similarly against this install, so this is likely not just app glue code

Primary objective:
- Build a deep, evidence-backed understanding of why packed Texture2D decode fails in this RimWorld/macOS build, and identify what does decode successfully (if anything).

Secondary objective:
- Determine whether `steel_a/b/c` are expected runtime variants for stack graphics, what their dimensions/formats are, and how RimWorld resolves them.

Do NOT optimize for quick hacks. Optimize for correctness, understanding, and reproducibility.

Key questions to answer:
1. Do any packed Texture2D assets decode successfully in this install? If yes, which ones and why those?
2. Are failures specific to certain formats, streaming modes, compression types, or missing typetree metadata?
3. What exactly are `steel_a`, `steel_b`, `steel_c` (class/type, dimensions, format, stream flags, mip info, source file)?
4. How does RimWorld choose among those variants at runtime (from decompiled game logic)?
5. Is there public documentation, issue reports, or known behavior in Unity asset tooling explaining this failure mode?

Investigation plan (execute in order):

Phase 1: Build a packed-asset fact base
- Enumerate a representative sample of packed Texture2D entries (at least 100) and capture:
  - name
  - class id/type
  - dimensions (if parseable)
  - texture format id/name
  - image_data size
  - stream data path/offset/size
  - source file / bundle location
- Split results into:
  - decodable
  - parseable metadata but undecodable
  - typetree parse failures / missing fields
- Produce a table with counts per failure category.

Phase 2: Steel-focused trace
- Trace `ThingDef` for `Steel` from XML into runtime graphic resolution expectations:
  - confirm `graphicClass` and path conventions
  - identify expected variant naming (`steel_a/b/c`) and selection logic
- Inspect packed entries for those names and report exact metadata (dimensions, format, stream info).

Phase 3: Tooling cross-check
- Reproduce with at least two independent codepaths/tools where possible:
  1) current repo code
  2) upstream `unity-asset-decode` examples
  3) optionally another community Unity extractor tool (if available locally)
- Compare outputs and isolate shared failure point.

Phase 4: RimWorld source correlation
- Use decompiled RimWorld source at `~/rimworld-decompiled/` to extract:
  - relevant graphic resolution classes (`Graphic_*`, `GraphicData`, material/texture retrieval)
  - any logic specific to stack/resource variants and naming
  - shader/material assumptions that may matter for extraction/decode
- Summarize behavior rules with symbol references.

Phase 5: Unity/format root-cause research
- Investigate whether this build uses stripped typetrees, streamed resource layouts, or format variants that require registry data (`.tpk`/json typetree).
- Find public docs/issues explaining:
  - `Invalid texture dimensions` in Unity extraction pipelines
  - cases where names are discoverable but decode fails
  - required metadata sources for successful decode

Deliverables (required):
1. `docs/investigation/packed-texture-root-cause.md`
   - Executive summary
   - Reproduction steps
   - Evidence tables
   - Root-cause hypothesis (or confirmed cause)
   - Confidence level and unknowns
2. `docs/investigation/steel-asset-trace.md`
   - End-to-end trace for `Steel`
   - Explanation of `steel_a/b/c` role
   - Expected dimensions/formats/selection behavior
3. `docs/investigation/recommended-next-steps.md`
   - Ranked implementation options with tradeoffs, e.g.:
     - typetree registry acquisition path
     - alternate parser/tooling path
     - pre-extraction pipeline
     - fallback strategy

Quality bar:
- Every key claim should cite concrete evidence (logs, symbol names, file paths, command outputs).
- Distinguish clearly between observed facts and hypotheses.
- Include at least one negative test (what was tried and did NOT explain the issue).

Useful local commands (adapt as needed):
- `cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel --no-window`
- `cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --search-packed-textures steel --search-limit 30`
- `cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --extract-packed-textures target/packed_textures`
- Upstream example baseline:
  - `cargo run --manifest-path "$HOME/.cargo/registry/src/index.crates.io-*/unity-asset-decode-0.2.0/Cargo.toml" --example export_textures --features texture-advanced -- "<resources.assets path>" "<output dir>"`

Final output format from you:
- concise summary paragraph
- bullet list of verified findings
- bullet list of unresolved questions
- recommended next actions (numbered)
