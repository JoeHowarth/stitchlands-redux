# stitchlands-redux

v0 prototype for loading a RimWorld core `ThingDef` and rendering a sprite with `wgpu`.

## Run

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/Data" --thingdef Steel
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

Search packed Texture2D names:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld" --search-packed-textures steel --search-limit 30
```

## Notes

- v0 supports `Graphic_Single`-style path resolution first.
- If a texture is missing, it renders a checkerboard fallback and logs a warning.
- Use `--texture-root <path>` (repeatable) to try extra directories for loose texture PNGs.
- Extra roots also support fuzzy filename lookup by basename (`Steel`, `Steel_south`, etc.) when exact `texPath` folders are not present.
- Packed Unity Texture2D lookup is attempted automatically after loose file lookup misses.
