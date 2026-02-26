# v1 Definition of Done

v1 is complete when map + things + pawns render from real RimWorld assets with stable ordering and repeatable checks.

## Acceptance Checklist

- Real terrain tiles render from RimWorld assets (no checker fallback in fixture path).
- Map fixture renders terrain + things + pawns in one scene.
- Draw ordering is deterministic across runs (terrain, then things, then pawns).
- Camera pan/zoom works in fixture and render commands.
- `fixture v1` works in headless mode with screenshot output.
- `tests/v1_smoke.rs` passes when local asset env vars are provided.
- `tests/v1_golden.rs` passes when local asset env vars are provided.
- README documents v1 fixture and debug workflows using current CLI subcommands.

## Required Commands

```bash
cargo run -- fixture v1
```

```bash
cargo run -- fixture v1 --no-window --screenshot target/v1_fixture.png
```

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v1_smoke -- --nocapture
```

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v1_golden -- --nocapture
```

## Out of Scope (v2+)

- Weather and overlay passes.
- Full RimWorld map section pipeline parity.
- Full pawn apparel/hediff edge-case parity for every loadout.
