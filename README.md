# stitchlands-redux

RimWorld asset renderer prototype in Rust + `wgpu`.

## Quick Start

Render a single thing:

```bash
cargo run -- render --thingdef Steel
```

Run a fixture scene (terrain + things + pawns + interactive runtime):

```bash
cargo run -- fixture fixtures/v2/move_lane.ron
```

Headless tick run:

```bash
cargo run -- fixture fixtures/v2/move_lane.ron --no-window --ticks 60
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

Fixture scene from RON:

```bash
cargo run -- fixture fixtures/v2/move_lane.ron
```

Override fixed-step timing:

```bash
cargo run -- fixture fixtures/v2/move_lane.ron --ticks 120 --fixed-dt 0.05
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

Probe pawn def decode coverage (body/head/hair/beard/apparel):

```bash
cargo run -- debug probe-defs
```

Diagnose texture roots:

```bash
cargo run -- debug diagnose-textures
```

Extract decodable packed textures:

```bash
cargo run -- debug extract-packed-textures target/packed_textures
```

## Smoke Tests

v0 (single-thing render):

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v0_smoke -- --nocapture
```

v2 (fixture scene build):

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v2_smoke -- --nocapture
```
