# stitchlands-redux

v0 prototype for loading a RimWorld core `ThingDef` and rendering a sprite with `wgpu`.

## Run

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/Data" --thingdef Steel
```

Example with overrides:

```bash
cargo run -- --rimworld-data "$HOME/Library/Application Support/Steam/steamapps/common/RimWorld/Data" --thingdef ChunkSlagSteel --cell-x 2 --cell-z 1 --scale 1.25 --tint 1,0.8,0.8,1
```

## Controls

- Pan: `WASD` or arrow keys
- Zoom: mouse wheel or `Q` / `E`

## Notes

- v0 supports `Graphic_Single`-style path resolution first.
- If a texture is missing, it renders a checkerboard fallback and logs a warning.
