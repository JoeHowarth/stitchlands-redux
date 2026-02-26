# stitchlands-redux

RimWorld asset renderer prototype in Rust + `wgpu`.

## Quick Start

Render a single thing:

```bash
cargo run -- render --thingdef Steel
```

Render v1 fixture scene (terrain + things + pawns):

```bash
cargo run -- fixture v1
```

Headless screenshot run:

```bash
cargo run -- fixture v1 --no-window --screenshot target/v1_fixture.png
```

## Default Config

You can omit repeated CLI paths by setting env vars:

- `STITCHLANDS_RIMWORLD_DATA`
- `STITCHLANDS_TYPETREE_REGISTRY`
- `STITCHLANDS_TEXTURE_ROOT`
- `STITCHLANDS_PACKED_DATA_ROOT`

Path-list env vars support `:` or `;` separators.

Example:

```bash
export STITCHLANDS_RIMWORLD_DATA="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld"
export STITCHLANDS_TYPETREE_REGISTRY="$HOME/path/to/typetree/lz4.tpk"
```

## Common Commands

Single render:

```bash
cargo run -- render --thingdef Steel --no-window
```

Sheet render for multiple defs:

```bash
cargo run -- render --thingdef Steel --extra-thingdef ChunkSlagSteel --extra-thingdef Plasteel --sheet-columns 3
```

V1 fixture (map + terrain + things + pawns):

```bash
cargo run -- fixture v1 --map-width 40 --map-height 40
```

Pawn-focused fixture:

```bash
cargo run -- fixture pawn --map-width 18 --map-height 18
```

Pawn loadout audit scene:

```bash
cargo run -- audit --pawn-count 10
```

## Debug Commands

List defs:

```bash
cargo run -- debug list-defs --def-filter steel --list-limit 20
```

Search packed texture names:

```bash
cargo run -- debug search-packed-textures steel --search-limit 30
```

Probe terrain decode coverage:

```bash
cargo run -- debug probe-terrain --terrain-probe-limit 64
```

Diagnose texture roots:

```bash
cargo run -- debug diagnose-textures
```

Extract decodable packed textures:

```bash
cargo run -- debug extract-packed-textures target/packed_textures
```

## Smoke / Regression Tests

v0 smoke:

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v0_smoke -- --nocapture
```

v1 smoke:

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v1_smoke -- --nocapture
```

v1 golden screenshot regression:

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v1_golden -- --nocapture
```

Golden image path: `tests/golden/v1_fixture_256.png`

Regenerate golden image intentionally:

```bash
cargo run -- fixture v1 --viewport-width 256 --viewport-height 256 --camera-zoom 8 --no-window --screenshot tests/golden/v1_fixture_256.png
```
