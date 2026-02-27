# Codebase Review Findings

## Facing Enum Fragmentation (3 copies)

The same concept — a cardinal direction — is defined in three places:
- `scene::PawnFacing` (v1 fixture map generator)
- `fixtures::PawnFacingSpec` (v2 RON schema)
- `pawn::PawnFacing` (composition pipeline)

This spawns three `map_facing()` conversion functions:
- `commands/v1_scene.rs:793` — `scene::PawnFacing` → `pawn::PawnFacing`
- `commands/fixture_v2_cmd.rs:361` — `fixtures::PawnFacingSpec` → `pawn::PawnFacing`
- `runtime/v2/mod.rs:238` — `fixtures::PawnFacingSpec` → `pawn::PawnFacing`

The last two are literally identical. `map_apparel_layer()` is similarly duplicated between `v1_scene.rs:866` and `fixture_v2_cmd.rs:350`.

## `scene.rs` is v1-only legacy

`scene.rs` exists solely to serve `commands/v1_scene.rs`. It defines its own `PawnFacing`, `PawnInstance`, `ThingInstance`, and `FixtureMap` types that duplicate concepts already in `fixtures/schema.rs` and `world/state.rs`. The v2 path doesn't use it at all. This is the root cause of the facing enum fragmentation — v1 and v2 use different schema types for the same thing.

## `build_v1_fixture_scene()` is 747 lines

`commands/v1_scene.rs:45-791`. It loads terrain, things, bodies, heads, hair, beards, apparel, generates a map, composes pawns, validates output, and writes trace files — all in one function. The extraction from main.rs was the right first step; the next step would be breaking it into phases.

## `DispatchContext` / `FixtureSceneConfig` field sprawl

`DispatchContext` has 11 fields (7 are def HashMaps). `FixtureSceneConfig` has 18 fields (8 are those same def HashMaps forwarded through). Every command construction site copies 7-8 fields from `DispatchContext` into `FixtureSceneConfig`. These def references could be grouped into a single `DefSet` struct.

## viewer.rs `expect()` calls on fallible operations

`viewer.rs:138` — `event_loop.create_window(attrs).expect("create window")`
`viewer.rs:153` — `Renderer::new(...).expect("create renderer")`
`viewer.rs:177` — `renderer.set_dynamic_instances(...).expect(...)`

These are real operations that can fail (GPU unavailable, bad window server, etc.). The winit `ApplicationHandler` trait's `resumed()` method returns `()` though, so propagating errors requires a different pattern (storing the error and exiting on next event, or using a channel). Not a quick fix but worth noting.

## Chooser function pattern in `fixture_v2_cmd.rs`

Four near-identical functions (`choose_body_def`, `choose_head_def`, `choose_hair_def`, `choose_beard_def` at lines 264-314) each do "if preferred exists return it, else return first by def_name." Could be a single generic.

## `scene.rs:38` — unchecked array index

`self.terrain[z * self.width + x]` — no bounds check. Callers (`v1_scene.rs`) iterate `0..map.width` and `0..map.height` so it's safe in practice, but the function is public and takes arbitrary `usize` arguments.

## Priority assessment

1. ~~**Facing enum unification** — removes 3 mapping functions, simplifies both pipelines~~ **Done** — unified into `pawn::PawnFacing` with serde derives; deleted `PawnFacingSpec`, `scene::PawnFacing`, and three `map_facing()` functions
2. ~~**DefSet struct** — groups the 7 def HashMaps, cuts DispatchContext/FixtureSceneConfig construction noise~~ **Done** — `DefSet<'a>` in `commands::common`, adopted by `DispatchContext` and `FixtureSceneConfig`
3. ~~**Duplicate `map_apparel_layer`** — quick win, extract to shared location~~ **Done** — replaced with `impl From<ApparelLayerDef> for ApparelLayer`
4. **`build_v1_fixture_scene` decomposition** — readability, but low urgency since it's stable code
