mod asset_resolver;
mod assets;
mod commands;
mod default_config;
mod defs;
mod packed_index;
mod packed_textures;
mod pawn;
mod renderer;
mod rimworld_paths;
mod scene;
mod typetree_registry;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use glam::{Vec2, Vec3};
use log::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::asset_resolver::AssetResolver;
use crate::commands::{
    diagnose_textures, list_defs, print_packed_texture_search, run_extract_packed_textures,
    run_terrain_probe,
};
use crate::default_config::{default_packed_index_path, merge_path_list, resolve_rimworld_input};
use crate::defs::{
    ApparelDef, ApparelLayerDef, BeardDefRender, BodyTypeDefRender, HairDefRender,
    HeadTypeDefRender, TerrainDef, ThingDef, load_apparel_defs, load_beard_defs,
    load_body_type_defs, load_hair_defs, load_head_type_defs, load_terrain_defs, load_thing_defs,
};
use crate::packed_index::PackedTextureIndex;
use crate::packed_textures::infer_packed_data_roots;
use crate::pawn::{
    ApparelLayer as ComposeApparelLayer, ApparelRenderInput, BeardTypeRenderData,
    BodyTypeRenderData, HeadTypeRenderData, HediffOverlayInput, OverlayAnchor, PawnComposeConfig,
    PawnDrawFlags, PawnFacing as ComposeFacing, PawnRenderInput, compose_pawn,
};
use crate::renderer::{Renderer, SpriteInput, SpriteParams};
use crate::rimworld_paths::resolve_data_dir;
use crate::scene::{
    count_terrain_families, generate_fixture_map, sorted_pawns, sorted_things_by_altitude,
};
use crate::typetree_registry::resolve_typetree_registry_paths;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(long)]
    rimworld_data: Option<PathBuf>,

    #[arg(long)]
    thingdef: Option<String>,

    #[arg(long)]
    image_path: Option<PathBuf>,

    #[arg(long)]
    extra_thingdef: Vec<String>,

    #[arg(long, default_value_t = false)]
    list_defs: bool,

    #[arg(long)]
    def_filter: Option<String>,

    #[arg(long, default_value_t = 25)]
    list_limit: usize,

    #[arg(long)]
    texture_root: Vec<PathBuf>,

    #[arg(long)]
    packed_data_root: Vec<PathBuf>,

    #[arg(long)]
    packed_index_path: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    rebuild_packed_index: bool,

    #[arg(long, default_value_t = false)]
    no_packed_index: bool,

    #[arg(long)]
    typetree_registry: Vec<PathBuf>,

    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    auto_typetree: bool,

    #[arg(long, default_value_t = 0)]
    packed_decode_probe: usize,

    #[arg(long, default_value_t = 8)]
    packed_decode_probe_min_attempts: usize,

    #[arg(long)]
    extract_packed_textures: Option<PathBuf>,

    #[arg(long)]
    search_packed_textures: Option<String>,

    #[arg(long, default_value_t = 20)]
    search_limit: usize,

    #[arg(long, default_value_t = false)]
    diagnose_textures: bool,

    #[arg(long, default_value_t = false)]
    probe_terrain: bool,

    #[arg(long, default_value_t = 64)]
    terrain_probe_limit: usize,

    #[arg(long, default_value_t = false)]
    scene_v1_fixture: bool,

    #[arg(long, default_value_t = false)]
    pawn_fixture: bool,

    #[arg(long, default_value_t = 40)]
    map_width: usize,

    #[arg(long, default_value_t = 40)]
    map_height: usize,

    #[arg(long)]
    export_resolved: Option<PathBuf>,

    #[arg(long)]
    screenshot: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    no_window: bool,

    #[arg(long, default_value_t = 0.0)]
    cell_x: f32,

    #[arg(long, default_value_t = 0.0)]
    cell_z: f32,

    #[arg(long, default_value_t = 1.0)]
    scale: f32,

    #[arg(long, value_parser = parse_tint, default_value = "1,1,1,1")]
    tint: [f32; 4],
}

fn parse_tint(input: &str) -> Result<[f32; 4], String> {
    let cleaned = input.replace(',', " ");
    let mut nums = cleaned
        .split_whitespace()
        .map(|v| v.parse::<f32>().map_err(|e| e.to_string()));
    let r = nums.next().ok_or_else(|| "missing r".to_string())??;
    let g = nums.next().ok_or_else(|| "missing g".to_string())??;
    let b = nums.next().ok_or_else(|| "missing b".to_string())??;
    let a = nums.next().transpose()?.unwrap_or(1.0);
    Ok([r, g, b, a])
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    let rimworld_input = resolve_rimworld_input(cli.rimworld_data.clone()).context(
        "could not resolve RimWorld path; set --rimworld-data or STITCHLANDS_RIMWORLD_DATA",
    )?;

    let data_dir = resolve_data_dir(&rimworld_input).with_context(|| {
        format!(
            "resolving rimworld data dir from {}",
            rimworld_input.display()
        )
    })?;
    info!("using RimWorld data dir: {}", data_dir.display());

    let texture_roots = merge_path_list(&cli.texture_root, "STITCHLANDS_TEXTURE_ROOT");
    let packed_root_overrides =
        merge_path_list(&cli.packed_data_root, "STITCHLANDS_PACKED_DATA_ROOT");

    let defs = load_thing_defs(&data_dir)
        .with_context(|| format!("loading defs from {}", data_dir.display()))?;
    info!("loaded {} thing defs with graphicData", defs.len());
    let terrain_defs = load_terrain_defs(&data_dir)
        .with_context(|| format!("loading terrain defs from {}", data_dir.display()))?;
    info!(
        "loaded {} terrain defs with texturePath",
        terrain_defs.len()
    );
    let apparel_defs = load_apparel_defs(&data_dir)
        .with_context(|| format!("loading apparel defs from {}", data_dir.display()))?;
    info!(
        "loaded {} apparel defs with graphicData",
        apparel_defs.len()
    );
    let body_type_defs = load_body_type_defs(&data_dir)
        .with_context(|| format!("loading body type defs from {}", data_dir.display()))?;
    info!("loaded {} body type defs", body_type_defs.len());
    let head_type_defs = load_head_type_defs(&data_dir)
        .with_context(|| format!("loading head type defs from {}", data_dir.display()))?;
    info!("loaded {} head type defs", head_type_defs.len());
    let beard_defs = load_beard_defs(&data_dir)
        .with_context(|| format!("loading beard defs from {}", data_dir.display()))?;
    info!("loaded {} beard defs", beard_defs.len());
    let hair_defs = load_hair_defs(&data_dir)
        .with_context(|| format!("loading hair defs from {}", data_dir.display()))?;
    info!("loaded {} hair defs", hair_defs.len());

    let mut packed_roots = infer_packed_data_roots(&rimworld_input, &data_dir);
    for extra in &packed_root_overrides {
        packed_roots.push(extra.clone());
    }
    packed_roots.sort();
    packed_roots.dedup();

    for explicit in &cli.typetree_registry {
        if !explicit.exists() {
            warn!(
                "typetree registry path does not exist and will be skipped: {}",
                explicit.display()
            );
        }
    }
    let typetree_registries =
        resolve_typetree_registry_paths(&cli.typetree_registry, cli.auto_typetree);
    // RimWorld 2022 macOS assets in this project are typetree-stripped; without an external
    // registry packed Texture2D parsing often falls back to invalid dimensions/format metadata.
    if typetree_registries.is_empty() {
        warn!(
            "no typetree registry selected; packed texture decode may fail on stripped Unity assets (set --typetree-registry or STITCHLANDS_TYPETREE_REGISTRY)"
        );
    } else {
        for registry in &typetree_registries {
            info!("using typetree registry: {}", registry.display());
        }
    }

    let packed_index = if cli.no_packed_index {
        None
    } else {
        let index_path = cli
            .packed_index_path
            .clone()
            .unwrap_or_else(default_packed_index_path);
        let index = PackedTextureIndex::load_or_build(
            &packed_roots,
            &typetree_registries,
            &index_path,
            cli.rebuild_packed_index,
        )?;
        if index.is_empty() {
            warn!(
                "packed texture metadata index is empty; packed lookup gating disabled for this run"
            );
        }
        Some(index)
    };

    let mut asset_resolver = AssetResolver::new(
        texture_roots,
        packed_roots,
        typetree_registries,
        packed_index,
    );

    if let Some(output_dir) = &cli.extract_packed_textures {
        run_extract_packed_textures(
            asset_resolver.packed_roots(),
            asset_resolver.typetree_registries(),
            output_dir,
        )?;
        info!(
            "extract command complete for output dir {}",
            output_dir.display()
        );
        return Ok(());
    }

    if let Some(query) = &cli.search_packed_textures {
        print_packed_texture_search(&asset_resolver, query, cli.search_limit);
        return Ok(());
    }

    if cli.diagnose_textures {
        diagnose_textures(
            &data_dir,
            asset_resolver.texture_roots(),
            asset_resolver.packed_roots(),
        );
        return Ok(());
    }

    if cli.list_defs {
        list_defs(&defs, cli.def_filter.as_deref(), cli.list_limit);
        return Ok(());
    }

    if let Some(image_path) = &cli.image_path {
        let image = image::open(image_path)
            .with_context(|| format!("loading image {}", image_path.display()))?
            .to_rgba8();
        info!("loaded direct image asset: {}", image_path.display());

        let sprite = RenderSprite {
            def_name: format!("image:{}", image_path.display()),
            image,
            params: SpriteParams {
                world_pos: Vec3::new(cli.cell_x + 0.5, cli.cell_z + 0.5, 0.0),
                size: Vec3::new(cli.scale, cli.scale, 0.0).truncate(),
                tint: cli.tint,
            },
            used_fallback: false,
        };

        if let Some(screenshot) = &cli.screenshot {
            info!("screenshot output: {}", screenshot.display());
        }
        if cli.no_window {
            if let Some(export_path) = &cli.export_resolved {
                sprite
                    .image
                    .save(export_path)
                    .with_context(|| format!("saving image to {}", export_path.display()))?;
                info!("wrote image export: {}", export_path.display());
            }
            return Ok(());
        }

        let mut app = App::new(vec![sprite], cli.screenshot, None);
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut app)?;
        return Ok(());
    }

    if cli.packed_decode_probe > 0
        && let Some(outcome) = asset_resolver.run_packed_decode_probe(
            cli.packed_decode_probe,
            cli.packed_decode_probe_min_attempts,
        )?
    {
        info!(
            "packed decode probe: attempted={} succeeded={}",
            outcome.attempted, outcome.succeeded
        );
        if outcome.disable_packed {
            warn!(
                "packed decode probe found 0 successful decodes in {} samples; disabling packed decode for this run",
                outcome.attempted
            );
            for sample in outcome.sample_errors {
                info!("packed decode probe sample failure: {sample}");
            }
            if !asset_resolver.has_typetree_registries() {
                warn!(
                    "remediation: provide --typetree-registry /path/to/registry.tpk (or set STITCHLANDS_TYPETREE_REGISTRY)"
                );
            }
        }
    }

    if cli.probe_terrain {
        run_terrain_probe(
            &data_dir,
            &terrain_defs,
            &mut asset_resolver,
            cli.terrain_probe_limit,
        )?;
        return Ok(());
    }

    if cli.scene_v1_fixture {
        let (render_sprites, camera_focus) = build_v1_fixture_scene(FixtureSceneConfig {
            data_dir: &data_dir,
            thing_defs: &defs,
            terrain_defs: &terrain_defs,
            apparel_defs: &apparel_defs,
            body_type_defs: &body_type_defs,
            head_type_defs: &head_type_defs,
            beard_defs: &beard_defs,
            hair_defs: &hair_defs,
            asset_resolver: &mut asset_resolver,
            width: cli.map_width,
            height: cli.map_height,
            pawn_focus_only: false,
        })?;
        if cli.no_window {
            return Ok(());
        }
        let mut app = App::new(render_sprites, cli.screenshot, Some(camera_focus));
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut app)?;
        return Ok(());
    }

    if cli.pawn_fixture {
        let (render_sprites, camera_focus) = build_v1_fixture_scene(FixtureSceneConfig {
            data_dir: &data_dir,
            thing_defs: &defs,
            terrain_defs: &terrain_defs,
            apparel_defs: &apparel_defs,
            body_type_defs: &body_type_defs,
            head_type_defs: &head_type_defs,
            beard_defs: &beard_defs,
            hair_defs: &hair_defs,
            asset_resolver: &mut asset_resolver,
            width: cli.map_width.clamp(8, 18),
            height: cli.map_height.clamp(8, 18),
            pawn_focus_only: true,
        })?;
        if cli.no_window {
            return Ok(());
        }
        let mut app = App::new(render_sprites, cli.screenshot, Some(camera_focus));
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut app)?;
        return Ok(());
    }

    let thingdef = cli
        .thingdef
        .as_deref()
        .context("--thingdef or --image-path is required unless --list-defs is used")?;
    let thing = defs
        .get(thingdef)
        .cloned()
        .with_context(|| make_missing_def_message(thingdef, &defs))?;
    info!("selected def: {}", thing.def_name);

    let mut selected_defs = vec![thing];
    for extra_name in &cli.extra_thingdef {
        let extra = defs
            .get(extra_name)
            .cloned()
            .with_context(|| make_missing_def_message(extra_name, &defs))?;
        info!("selected extra def: {}", extra.def_name);
        selected_defs.push(extra);
    }

    let mut render_sprites = Vec::with_capacity(selected_defs.len());
    for (index, selected) in selected_defs.iter().enumerate() {
        let resolved = asset_resolver
            .resolve_thing(&data_dir, selected)
            .with_context(|| {
                format!(
                    "resolving texture for def '{}' path '{}'",
                    selected.def_name, selected.graphic_data.tex_path
                )
            })?;
        let sprite_asset = resolved.sprite;
        let resolved_from_packed = resolved.resolved_from_packed;

        if sprite_asset.used_fallback {
            if asset_resolver.can_try_packed(&selected.graphic_data.tex_path) {
                if let Some(probe) = asset_resolver
                    .maybe_probe_decode_candidates(&selected.graphic_data.tex_path, 8)?
                    && probe.attempted > 0
                {
                    warn!(
                        "packed texture probe for '{}' attempted {} candidates, {} decodable",
                        selected.def_name, probe.attempted, probe.succeeded
                    );
                    for (name, err) in probe.sample_errors {
                        info!("packed candidate '{}' failed decode: {}", name, err);
                    }
                    if probe.succeeded == 0 {
                        warn!(
                            "no decodable packed candidates for '{}'; this usually means stripped/missing TypeTree metadata for this Unity build",
                            selected.def_name
                        );
                    }
                }
            } else {
                info!(
                    "packed index has no candidate for '{}'; skipping packed decode",
                    selected.graphic_data.tex_path
                );
            }
            warn!(
                "texture missing for '{}' ({}) - using checker fallback",
                selected.def_name, selected.graphic_data.tex_path
            );
            for attempted in sprite_asset.attempted_paths.iter().take(6) {
                info!("attempted: {}", attempted.display());
            }
        }

        if let Some(path) = &sprite_asset.source_path {
            if resolved_from_packed {
                info!("resolved texture (packed): {}", path.display());
            } else if sprite_asset.resolved_with_fuzzy_match {
                info!("resolved texture (fuzzy): {}", path.display());
            } else {
                info!("resolved texture: {}", path.display());
            }
        }

        if let Some(export_path) = &cli.export_resolved {
            let with_suffix = if selected_defs.len() == 1 {
                export_path.clone()
            } else {
                let stem = export_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("resolved");
                let ext = export_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let filename = format!("{stem}_{}_{}.{}", index, selected.def_name, ext);
                export_path.with_file_name(filename)
            };
            sprite_asset
                .image
                .save(&with_suffix)
                .with_context(|| format!("saving resolved sprite to {}", with_suffix.display()))?;
            info!("wrote resolved sprite image: {}", with_suffix.display());
        }

        let size = selected.graphic_data.draw_size * cli.scale;
        let draw_offset = selected.graphic_data.draw_offset;
        let world_pos = Vec3::new(
            cli.cell_x + index as f32 * 1.75 + 0.5 + draw_offset.x,
            cli.cell_z + 0.5 + draw_offset.z,
            draw_offset.y,
        );
        let tint = [
            cli.tint[0] * selected.graphic_data.color.r,
            cli.tint[1] * selected.graphic_data.color.g,
            cli.tint[2] * selected.graphic_data.color.b,
            cli.tint[3] * selected.graphic_data.color.a,
        ];

        info!(
            "sprite params [{}] {} -> size=({:.2}, {:.2}) offset=({:.2}, {:.2}, {:.2})",
            index, selected.def_name, size.x, size.y, draw_offset.x, draw_offset.y, draw_offset.z
        );

        render_sprites.push(RenderSprite {
            def_name: selected.def_name.clone(),
            image: sprite_asset.image,
            params: SpriteParams {
                world_pos,
                size,
                tint,
            },
            used_fallback: sprite_asset.used_fallback,
        });
    }

    if cli.no_window {
        return Ok(());
    }

    let mut app = App::new(render_sprites, cli.screenshot, None);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct FixtureSceneConfig<'a> {
    data_dir: &'a Path,
    thing_defs: &'a std::collections::HashMap<String, ThingDef>,
    terrain_defs: &'a std::collections::HashMap<String, TerrainDef>,
    apparel_defs: &'a std::collections::HashMap<String, ApparelDef>,
    body_type_defs: &'a std::collections::HashMap<String, BodyTypeDefRender>,
    head_type_defs: &'a std::collections::HashMap<String, HeadTypeDefRender>,
    beard_defs: &'a std::collections::HashMap<String, BeardDefRender>,
    hair_defs: &'a std::collections::HashMap<String, HairDefRender>,
    asset_resolver: &'a mut AssetResolver,
    width: usize,
    height: usize,
    pawn_focus_only: bool,
}

fn make_missing_def_message(
    thingdef: &str,
    defs: &std::collections::HashMap<String, ThingDef>,
) -> String {
    let mut suggestions: Vec<&str> = defs
        .keys()
        .filter_map(|name| {
            if name.eq_ignore_ascii_case(thingdef) {
                Some(name.as_str())
            } else {
                let name_lower = name.to_lowercase();
                let query_lower = thingdef.to_lowercase();
                if name_lower.contains(&query_lower) || query_lower.contains(&name_lower) {
                    Some(name.as_str())
                } else {
                    None
                }
            }
        })
        .take(5)
        .collect();
    suggestions.sort_unstable();

    if suggestions.is_empty() {
        format!("thingdef '{thingdef}' not found")
    } else {
        format!(
            "thingdef '{thingdef}' not found. Close matches: {}",
            suggestions.join(", ")
        )
    }
}

fn build_v1_fixture_scene(config: FixtureSceneConfig<'_>) -> Result<(Vec<RenderSprite>, Vec2)> {
    let FixtureSceneConfig {
        data_dir,
        thing_defs,
        terrain_defs,
        apparel_defs,
        body_type_defs,
        head_type_defs,
        beard_defs,
        hair_defs,
        asset_resolver,
        width,
        height,
        pawn_focus_only,
    } = config;

    let mut terrain_rows: Vec<_> = terrain_defs.values().collect();
    terrain_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));

    let mut chosen_terrain: Vec<(String, image::RgbaImage)> = Vec::new();
    for terrain in terrain_rows {
        let resolved =
            asset_resolver.resolve_texture_path(data_dir, terrain.texture_path.as_str())?;
        if resolved.sprite.used_fallback {
            continue;
        }
        chosen_terrain.push((terrain.def_name.clone(), resolved.sprite.image));
        if chosen_terrain.len() >= 3 {
            break;
        }
    }
    if chosen_terrain.len() < 3 {
        anyhow::bail!("v1 fixture needs at least 3 decodable terrain defs");
    }

    let preferred_things = [
        "Steel",
        "ChunkSlagSteel",
        "Plasteel",
        "WoodLog",
        "ComponentIndustrial",
    ];
    let mut thing_choices = Vec::new();
    for name in preferred_things {
        if let Some(def) = thing_defs.get(name) {
            let resolved = asset_resolver.resolve_thing(data_dir, def)?;
            if resolved.sprite.used_fallback {
                continue;
            }
            thing_choices.push((def.clone(), resolved.sprite.image));
        }
    }
    if thing_choices.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable ThingDef");
    }

    let mut body_rows: Vec<_> = body_type_defs.values().collect();
    body_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_body_choices: Vec<(BodyTypeDefRender, image::RgbaImage)> = Vec::new();
    for body in body_rows {
        let resolved =
            asset_resolver.resolve_texture_path(data_dir, &body.body_naked_graphic_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_body_choices.push((body.clone(), resolved.sprite.image));
    }
    if pawn_body_choices.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable pawn body texture");
    }

    let mut head_rows: Vec<_> = head_type_defs.values().collect();
    head_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_head_choices: Vec<(HeadTypeDefRender, image::RgbaImage)> = Vec::new();
    for head in head_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, &head.graphic_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_head_choices.push((head.clone(), resolved.sprite.image));
    }

    let mut hair_rows: Vec<_> = hair_defs.values().collect();
    hair_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_hair_choices: Vec<(HairDefRender, image::RgbaImage)> = Vec::new();
    for hair in hair_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, &hair.tex_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_hair_choices.push((hair.clone(), resolved.sprite.image));
    }

    let mut beard_rows: Vec<_> = beard_defs
        .values()
        .filter(|b| !b.no_graphic && !b.tex_path.is_empty())
        .collect();
    beard_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_beard_choices: Vec<(BeardDefRender, image::RgbaImage)> = Vec::new();
    for beard in beard_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, &beard.tex_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_beard_choices.push((beard.clone(), resolved.sprite.image));
    }

    let mut apparel_rows: Vec<_> = apparel_defs.values().collect();
    apparel_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut chosen_apparel: Vec<(ApparelDef, image::RgbaImage)> = Vec::new();
    let mut picked_body_layer = false;
    let mut picked_shell_layer = false;
    let mut picked_head_layer = false;
    for apparel in apparel_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, apparel.tex_path.as_str())?;
        if resolved.sprite.used_fallback {
            continue;
        }

        let is_body = matches!(
            apparel.layer,
            ApparelLayerDef::OnSkin | ApparelLayerDef::Middle
        );
        let is_shellish = matches!(
            apparel.layer,
            ApparelLayerDef::Shell | ApparelLayerDef::Belt
        );
        let is_head = matches!(
            apparel.layer,
            ApparelLayerDef::Overhead | ApparelLayerDef::EyeCover
        );

        if is_body && !picked_body_layer {
            picked_body_layer = true;
            chosen_apparel.push((apparel.clone(), resolved.sprite.image));
        } else if is_shellish && !picked_shell_layer {
            picked_shell_layer = true;
            chosen_apparel.push((apparel.clone(), resolved.sprite.image));
        } else if is_head && !picked_head_layer {
            picked_head_layer = true;
            chosen_apparel.push((apparel.clone(), resolved.sprite.image));
        }

        if picked_body_layer && picked_shell_layer && picked_head_layer {
            break;
        }
    }
    if chosen_apparel.is_empty() {
        warn!("v1 fixture found no decodable apparel layers; pawns will be unclothed");
    } else if pawn_focus_only && chosen_apparel.len() < 2 {
        anyhow::bail!(
            "pawn fixture requires at least two decodable apparel layers (body + shell/head)"
        );
    }

    let terrain_names = [
        chosen_terrain[0].0.as_str(),
        chosen_terrain[1].0.as_str(),
        chosen_terrain[2].0.as_str(),
    ];
    let thing_names: Vec<String> = if pawn_focus_only {
        Vec::new()
    } else {
        thing_choices
            .iter()
            .take(20)
            .map(|(def, _)| def.def_name.clone())
            .collect()
    };
    let pawn_tex: Vec<String> = if pawn_focus_only {
        pawn_body_choices
            .iter()
            .take(1)
            .map(|(body, _)| body.body_naked_graphic_path.clone())
            .collect()
    } else {
        pawn_body_choices
            .iter()
            .take(6)
            .map(|(body, _)| body.body_naked_graphic_path.clone())
            .collect()
    };
    let map = generate_fixture_map(
        width.max(8),
        height.max(8),
        terrain_names,
        &thing_names,
        &pawn_tex,
    );

    let mut terrain_by_name = std::collections::HashMap::new();
    for (name, image) in chosen_terrain {
        terrain_by_name.insert(name, image);
    }
    let mut thing_by_name = std::collections::HashMap::new();
    for (def, image) in thing_choices {
        thing_by_name.insert(def.def_name.clone(), (def, image));
    }
    let mut body_by_tex = std::collections::HashMap::new();
    for (body, _) in &pawn_body_choices {
        body_by_tex.insert(body.body_naked_graphic_path.clone(), body.clone());
    }
    let mut head_by_tex = std::collections::HashMap::new();
    for (head, _) in &pawn_head_choices {
        head_by_tex.insert(head.graphic_path.clone(), head.clone());
    }
    let mut beard_by_tex = std::collections::HashMap::new();
    for (beard, _) in &pawn_beard_choices {
        beard_by_tex.insert(beard.tex_path.clone(), beard.clone());
    }
    let mut pawn_layer_by_tex = std::collections::HashMap::new();
    for (tex_path, image) in pawn_body_choices
        .into_iter()
        .map(|(body, image)| (body.body_naked_graphic_path, image))
        .chain(
            pawn_head_choices
                .into_iter()
                .map(|(head, image)| (head.graphic_path, image)),
        )
        .chain(
            pawn_hair_choices
                .into_iter()
                .map(|(hair, image)| (hair.tex_path, image)),
        )
        .chain(
            pawn_beard_choices
                .into_iter()
                .map(|(beard, image)| (beard.tex_path, image)),
        )
        .chain(
            chosen_apparel
                .iter()
                .map(|(apparel, image)| (apparel.tex_path.clone(), image.clone())),
        )
    {
        pawn_layer_by_tex.insert(tex_path, image);
    }
    let mut head_tex_paths: Vec<String> = pawn_layer_by_tex
        .keys()
        .filter(|path| path.contains("/Heads/"))
        .cloned()
        .collect();
    head_tex_paths.sort();
    let mut hair_tex_paths: Vec<String> = pawn_layer_by_tex
        .keys()
        .filter(|path| path.contains("/Hairs/"))
        .cloned()
        .collect();
    hair_tex_paths.sort();
    let mut beard_tex_paths: Vec<String> = pawn_layer_by_tex
        .keys()
        .filter(|path| path.contains("/Beards/"))
        .cloned()
        .collect();
    beard_tex_paths.sort();

    let mut sprites =
        Vec::with_capacity(map.width * map.height + map.things.len() + map.pawns.len());
    for z in 0..map.height {
        for x in 0..map.width {
            let name = map.terrain_at(x, z);
            let Some(image) = terrain_by_name.get(name) else {
                continue;
            };
            sprites.push(RenderSprite {
                def_name: format!("Terrain::{name}"),
                image: image.clone(),
                params: SpriteParams {
                    world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -1.0),
                    size: Vec2::new(1.0, 1.0),
                    tint: [1.0, 1.0, 1.0, 1.0],
                },
                used_fallback: false,
            });
        }
    }

    for thing in sorted_things_by_altitude(&map.things) {
        let Some((def, image)) = thing_by_name.get(&thing.def_name) else {
            continue;
        };
        let draw_offset = def.graphic_data.draw_offset;
        let thing_pos = Vec3::new(
            thing.cell_x as f32 + 0.5 + draw_offset.x,
            thing.cell_z as f32 + 0.5 + draw_offset.z,
            -0.8 + draw_offset.y * 0.01,
        );
        let thing_size = Vec2::new(
            def.graphic_data.draw_size.x.max(1.1),
            def.graphic_data.draw_size.y.max(1.1),
        );
        sprites.push(RenderSprite {
            def_name: format!("Thing::{}", def.def_name),
            image: image.clone(),
            params: SpriteParams {
                world_pos: thing_pos,
                size: thing_size,
                tint: [
                    def.graphic_data.color.r,
                    def.graphic_data.color.g,
                    def.graphic_data.color.b,
                    def.graphic_data.color.a,
                ],
            },
            used_fallback: false,
        });
    }

    for pawn in sorted_pawns(&map.pawns) {
        let head_tex = if head_tex_paths.is_empty() {
            None
        } else {
            Some(head_tex_paths[pawn.label.len() % head_tex_paths.len()].clone())
        };
        let hair_tex = hair_tex_paths.first().cloned();
        let beard_tex = beard_tex_paths.first().cloned();
        let body_render = body_by_tex.get(&pawn.tex_path);
        let head_render = head_tex
            .as_ref()
            .and_then(|tex| head_by_tex.get(tex))
            .cloned();
        let beard_render = beard_tex
            .as_ref()
            .and_then(|tex| beard_by_tex.get(tex))
            .cloned();
        let apparel_inputs: Vec<ApparelRenderInput> = chosen_apparel
            .iter()
            .map(|(apparel, _)| ApparelRenderInput {
                label: apparel.def_name.clone(),
                tex_path: apparel.tex_path.clone(),
                layer: map_apparel_layer(apparel.layer),
                covers_upper_head: apparel.covers_upper_head,
                covers_full_head: apparel.covers_full_head,
                draw_size: apparel.draw_size,
                tint: [
                    apparel.color.r,
                    apparel.color.g,
                    apparel.color.b,
                    apparel.color.a,
                ],
            })
            .collect();
        let hediff_overlays = vec![
            HediffOverlayInput {
                label: "TorsoScar".to_string(),
                tex_path: pawn.tex_path.clone(),
                anchor: OverlayAnchor::Body,
                layer_offset: 1,
                draw_size: Vec2::new(0.75, 0.75),
                tint: [1.0, 0.45, 0.45, 0.70],
                required_body_part_group: Some("Torso".to_string()),
                visible_facing: Some(vec![ComposeFacing::South, ComposeFacing::East]),
            },
            HediffOverlayInput {
                label: "FaceBruise".to_string(),
                tex_path: head_tex.clone().unwrap_or_else(|| pawn.tex_path.clone()),
                anchor: OverlayAnchor::Head,
                layer_offset: 1,
                draw_size: Vec2::new(0.6, 0.6),
                tint: [0.75, 0.25, 0.25, 0.60],
                required_body_part_group: Some("UpperHead".to_string()),
                visible_facing: None,
            },
        ];
        let compose_input = PawnRenderInput {
            label: pawn.label.clone(),
            facing: map_facing(pawn.facing),
            world_pos: Vec3::new(pawn.cell_x as f32 + 0.5, pawn.cell_z as f32 + 0.5, 0.0),
            body_tex_path: pawn.tex_path.clone(),
            head_tex_path: head_tex,
            stump_tex_path: None,
            hair_tex_path: hair_tex,
            beard_tex_path: beard_tex,
            body_size: body_render
                .map(|b| b.body_graphic_scale)
                .unwrap_or(Vec2::new(1.0, 1.0)),
            head_size: head_render
                .as_ref()
                .map(|_| Vec2::new(1.0, 1.0))
                .unwrap_or(Vec2::new(1.0, 1.0)),
            stump_size: Vec2::new(0.8, 0.8),
            hair_size: head_render
                .as_ref()
                .map(|h| h.hair_mesh_size)
                .unwrap_or(Vec2::new(1.5, 1.5)),
            beard_size: head_render
                .as_ref()
                .map(|h| h.beard_mesh_size)
                .unwrap_or(Vec2::new(1.5, 1.5)),
            body_type: BodyTypeRenderData {
                head_offset: body_render
                    .map(|b| b.head_offset)
                    .unwrap_or(Vec2::new(0.0, 0.34)),
                body_size_factor: 1.0,
            },
            head_type: head_render
                .as_ref()
                .map(|h| HeadTypeRenderData {
                    narrow: h.narrow,
                    narrow_crown_horizontal_offset: 0.0,
                    beard_offset: h.beard_offset,
                    beard_offset_x_east: h.beard_offset_x_east,
                })
                .unwrap_or_default(),
            beard_type: beard_render
                .as_ref()
                .map(|b| BeardTypeRenderData {
                    offset_narrow_east: b.offset_narrow_east,
                    offset_narrow_south: b.offset_narrow_south,
                })
                .unwrap_or_default(),
            tint: [1.0, 1.0, 1.0, 1.0],
            apparel: apparel_inputs,
            present_body_part_groups: vec![
                "Torso".to_string(),
                "UpperHead".to_string(),
                "Eyes".to_string(),
            ],
            hediff_overlays,
            draw_flags: PawnDrawFlags::NONE,
        };
        let composed = compose_pawn(&compose_input, &PawnComposeConfig::default());

        let body_path = &compose_input.body_tex_path;
        if !pawn_layer_by_tex.contains_key(body_path) {
            continue;
        }

        for node in composed.nodes {
            let Some(image) = pawn_layer_by_tex.get(&node.tex_path) else {
                continue;
            };
            sprites.push(RenderSprite {
                def_name: format!("Pawn::{}::{:?}::{}", pawn.label, node.kind, node.id),
                image: image.clone(),
                params: SpriteParams {
                    world_pos: node.world_pos,
                    size: node.size,
                    tint: node.tint,
                },
                used_fallback: false,
            });
        }
    }

    let camera_focus = if let Some(thing) = map.things.first() {
        Vec2::new(thing.cell_x as f32 + 0.5, thing.cell_z as f32 + 0.5)
    } else if let Some(pawn) = map.pawns.first() {
        Vec2::new(pawn.cell_x as f32 + 0.5, pawn.cell_z as f32 + 0.5)
    } else {
        Vec2::new(map.width as f32 * 0.5, map.height as f32 * 0.5)
    };

    if pawn_focus_only {
        info!(
            "pawn fixture scene built: map={}x{} terrain_families={} pawns={} drawables={}",
            map.width,
            map.height,
            count_terrain_families(&map),
            map.pawns.len(),
            sprites.len()
        );
    } else {
        info!(
            "v1 fixture scene built: map={}x{} terrain_families={} tiles={} things={} pawns={} drawables={} zbands=terrain(-1.0),thing(~-0.8),pawn(~-0.6)",
            map.width,
            map.height,
            count_terrain_families(&map),
            map.width * map.height,
            map.things.len(),
            map.pawns.len(),
            sprites.len()
        );
    }
    Ok((sprites, camera_focus))
}

fn map_facing(facing: scene::PawnFacing) -> ComposeFacing {
    match facing {
        scene::PawnFacing::North => ComposeFacing::North,
        scene::PawnFacing::East => ComposeFacing::East,
        scene::PawnFacing::South => ComposeFacing::South,
        scene::PawnFacing::West => ComposeFacing::West,
    }
}

fn map_apparel_layer(layer: ApparelLayerDef) -> ComposeApparelLayer {
    match layer {
        ApparelLayerDef::OnSkin => ComposeApparelLayer::OnSkin,
        ApparelLayerDef::Middle => ComposeApparelLayer::Middle,
        ApparelLayerDef::Shell => ComposeApparelLayer::Shell,
        ApparelLayerDef::Belt => ComposeApparelLayer::Belt,
        ApparelLayerDef::Overhead => ComposeApparelLayer::Overhead,
        ApparelLayerDef::EyeCover => ComposeApparelLayer::EyeCover,
    }
}

struct App {
    sprites: Vec<RenderSprite>,
    screenshot_path: Option<PathBuf>,
    initial_camera_center: Option<Vec2>,
    screenshot_taken: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}

struct RenderSprite {
    def_name: String,
    image: image::RgbaImage,
    params: SpriteParams,
    used_fallback: bool,
}

impl App {
    fn new(
        sprites: Vec<RenderSprite>,
        screenshot_path: Option<PathBuf>,
        initial_camera_center: Option<Vec2>,
    ) -> Self {
        Self {
            sprites,
            screenshot_path,
            initial_camera_center,
            screenshot_taken: false,
            window: None,
            renderer: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let first = self
            .sprites
            .first()
            .expect("at least one sprite exists in app state");
        let fallback_count = self.sprites.iter().filter(|s| s.used_fallback).count();
        let attrs = Window::default_attributes().with_title(format!(
            "stitchlands-redux | sprites={} first={} fallback={} | pan: WASD/Arrows zoom: wheel/QE",
            self.sprites.len(),
            first.def_name,
            fallback_count
        ));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let sprite_inputs = self
            .sprites
            .drain(..)
            .map(|sprite| SpriteInput {
                image: sprite.image,
                params: sprite.params,
            })
            .collect();
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            sprite_inputs,
            self.initial_camera_center,
        ))
        .expect("create renderer");

        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        if renderer.input(&event) {
            window.request_redraw();
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::RedrawRequested => {
                let capture = if self.screenshot_taken {
                    None
                } else {
                    self.screenshot_path.as_deref()
                };
                match renderer.render(capture) {
                    Ok(captured) => {
                        if captured {
                            self.screenshot_taken = true;
                            event_loop.exit();
                        }
                    }
                    Err(err) => {
                        if let Some(surface_err) = err.downcast_ref::<wgpu::SurfaceError>() {
                            if let Err(handle_err) = renderer.handle_surface_error(surface_err) {
                                eprintln!("render error: {handle_err:#}");
                                event_loop.exit();
                            }
                        } else {
                            eprintln!("render error: {err:#}");
                            event_loop.exit();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}
