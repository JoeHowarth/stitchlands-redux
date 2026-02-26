# stitchlands-redux

Prototype for loading RimWorld defs/assets and rendering sprites/scenes with `wgpu`.

## Run

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/Data" --thingdef Steel
```

If your install is in the default Steam location, `--rimworld-data` can be omitted:

```bash
cargo run -- --thingdef Steel
```

You can also set a persistent default:

```bash
export STITCHLANDS_RIMWORLD_DATA="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld"
```

On macOS Steam installs, you can also pass install root and it auto-resolves:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel
```

Example with overrides:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/Data" --thingdef ChunkSlagSteel --cell-x 2 --cell-z 1 --scale 1.25 --tint 1,0.8,0.8,1
```

Show multiple defs side-by-side:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel --extra-thingdef ChunkSlagSteel --extra-thingdef Plasteel
```

Render a known loose game image directly (useful sanity check):

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --image-path "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/RimWorldMac.app/Data/Core/About/Preview.png"
```

## Controls

- Pan: `WASD` or arrow keys
- Zoom: mouse wheel or `Q` / `E`

## Debug Commands

List thing defs with a filter:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --list-defs --def-filter steel --list-limit 20
```

Resolve one def to an image file without opening a window:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel --no-window --export-resolved target/steel_resolved.png
```

Capture a rendered screenshot and exit:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel --screenshot target/frame.png
```

Probe terrain decode coverage (loose + packed resolver):

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --probe-terrain --terrain-probe-limit 60
```

Launch the v1 fixture scene (terrain tilemap + things + pawns):

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --typetree-registry /path/to/typetree.tpk --scene-v1-fixture
```

`--packed-decode-probe` is now opt-in (default `0`) for faster startup.  
Use it only when debugging decode health:

```bash
cargo run -- --scene-v1-fixture --packed-decode-probe 24
```

Packed texture metadata index (names/container paths) is cached on disk to speed repeated runs.
Defaults to `$HOME/.cache/stitchlands-redux/packed_texture_index_v1.txt`.

```bash
# force a rebuild
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --rebuild-packed-index --search-packed-textures steel

# custom cache location
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --packed-index-path /tmp/stitchlands-index.txt --scene-v1-fixture
```

Check whether this install has loose texture PNGs:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --diagnose-textures
```

Try packed Unity data roots explicitly (if auto-detect misses your layout):

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel --packed-data-root "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/RimWorldMac.app/Contents/Resources/Data"
```

Extract decodable Texture2D images from packed Unity data:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --extract-packed-textures target/packed_textures
```

If you have a Unity TypeTree registry (`.json` or `.tpk`), pass it for better packed decode coverage:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --thingdef Steel --typetree-registry /path/to/typetree.tpk
```

`--typetree-registry` also accepts a directory (it will recursively load `*.tpk` and `*.json`).
You can set a path list via `STITCHLANDS_TYPETREE_REGISTRY` as an alternative.

`--texture-root` and `--packed-data-root` also support env defaults:
- `STITCHLANDS_TEXTURE_ROOT`
- `STITCHLANDS_PACKED_DATA_ROOT`

Both env vars accept path lists separated by `:` or `;`.

Search packed Texture2D names:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --search-packed-textures steel --search-limit 30
```

## Notes

- Thing and terrain resolution use loose lookup first, then packed Unity lookup.
- If a texture is missing, it renders a checkerboard fallback and logs a warning.
- Use `--texture-root <path>` (repeatable) to try extra directories for loose texture PNGs.
- Extra roots also support fuzzy filename lookup by basename (`Steel`, `Steel_south`, etc.) when exact `texPath` folders are not present.
- Packed Unity Texture2D lookup is attempted automatically after loose file lookup misses.
- Scene draw order is deterministic: terrain first, then things, then pawns.

## Smoke Test

Run the Steel no-fallback smoke test (requires local RimWorld install + typetree registry):

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v0_smoke -- --nocapture
```

Run the v1 fixture scene smoke test:

```bash
RIMWORLD_DATA_DIR="$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" \
RIMWORLD_TYPETREE_REGISTRY="/path/to/typetree.tpk" \
cargo test --test v1_smoke -- --nocapture
```
