mod assets;
mod default_config;
mod defs;
mod packed_index;
mod packed_textures;
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
use walkdir::WalkDir;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::assets::{SpriteAsset, resolve_sprite, resolve_texture_path};
use crate::default_config::{default_packed_index_path, merge_path_list, resolve_rimworld_input};
use crate::defs::{TerrainDef, ThingDef, load_terrain_defs, load_thing_defs};
use crate::packed_index::PackedTextureIndex;
use crate::packed_textures::{
    PackedTextureResolver, extract_all_packed_textures, infer_packed_data_roots,
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

struct PackedResolverState {
    packed_roots: Vec<PathBuf>,
    typetree_registries: Vec<PathBuf>,
    resolver: Option<PackedTextureResolver>,
    build_attempted: bool,
    build_fn: Box<BuildPackedResolverFn>,
}

type BuildPackedResolverFn =
    dyn FnMut(&[PathBuf], &[PathBuf]) -> Result<Option<PackedTextureResolver>>;

impl PackedResolverState {
    fn new(packed_roots: Vec<PathBuf>, typetree_registries: Vec<PathBuf>) -> Self {
        Self {
            packed_roots,
            typetree_registries,
            resolver: None,
            build_attempted: false,
            build_fn: Box::new(PackedTextureResolver::build),
        }
    }

    #[cfg(test)]
    fn with_builder(
        packed_roots: Vec<PathBuf>,
        typetree_registries: Vec<PathBuf>,
        build_fn: Box<BuildPackedResolverFn>,
    ) -> Self {
        Self {
            packed_roots,
            typetree_registries,
            resolver: None,
            build_attempted: false,
            build_fn,
        }
    }

    fn get(&mut self) -> Result<Option<&PackedTextureResolver>> {
        if !self.build_attempted {
            self.resolver = (self.build_fn)(&self.packed_roots, &self.typetree_registries)?;
            self.build_attempted = true;
            if let Some(resolver) = self.resolver.as_ref() {
                for root in resolver.loaded_roots() {
                    info!("packed resolver root: {}", root.display());
                }
            }
        }
        Ok(self.resolver.as_ref())
    }

    fn disable(&mut self) {
        self.build_attempted = true;
        self.resolver = None;
    }
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

    let mut packed_resolver_state =
        PackedResolverState::new(packed_roots.clone(), typetree_registries.clone());

    if let Some(output_dir) = &cli.extract_packed_textures {
        let summary = extract_all_packed_textures(&packed_roots, &typetree_registries, output_dir)
            .with_context(|| format!("extracting packed textures into {}", output_dir.display()))?;
        info!(
            "packed texture extraction finished: scanned={} exported={} failed={}",
            summary.scanned_textures, summary.exported_textures, summary.failed_textures
        );
        return Ok(());
    }

    if let Some(query) = &cli.search_packed_textures {
        if let Some(index) = packed_index.as_ref() {
            let matches = index.search_names(query, cli.search_limit);
            for name in &matches {
                println!("{name}");
            }
            println!("matched {} packed texture names", matches.len());
        } else {
            println!("no packed texture index loaded");
        }
        return Ok(());
    }

    if cli.diagnose_textures {
        diagnose_textures(&data_dir, &texture_roots);
        let packed_roots = infer_packed_data_roots(&rimworld_input, &data_dir);
        for root in packed_roots {
            println!(
                "packed candidate: {} | exists={}",
                root.display(),
                root.exists()
            );
        }
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

        let mut app = App::new(vec![sprite], cli.screenshot);
        let event_loop = EventLoop::new()?;
        event_loop.run_app(&mut app)?;
        return Ok(());
    }

    if cli.packed_decode_probe > 0 {
        let mut disable_packed = false;
        if let Some(resolver) = packed_resolver_state.get()? {
            let health = resolver.decode_health_sample(cli.packed_decode_probe);
            if health.attempted >= cli.packed_decode_probe_min_attempts {
                info!(
                    "packed decode probe: attempted={} succeeded={}",
                    health.attempted, health.succeeded
                );
                if health.succeeded == 0 {
                    warn!(
                        "packed decode probe found 0 successful decodes in {} samples; disabling packed decode for this run",
                        health.attempted
                    );
                    for sample in health.sample_errors {
                        info!("packed decode probe sample failure: {sample}");
                    }
                    if typetree_registries.is_empty() {
                        warn!(
                            "remediation: provide --typetree-registry /path/to/registry.tpk (or set STITCHLANDS_TYPETREE_REGISTRY)"
                        );
                    }
                    disable_packed = true;
                }
            }
        }
        if disable_packed {
            packed_resolver_state.disable();
        }
    }

    if cli.probe_terrain {
        run_terrain_probe(
            &data_dir,
            &terrain_defs,
            &texture_roots,
            packed_index.as_ref(),
            &mut packed_resolver_state,
            cli.terrain_probe_limit,
        )?;
        return Ok(());
    }

    if cli.scene_v1_fixture {
        let render_sprites = build_v1_fixture_scene(FixtureSceneConfig {
            data_dir: &data_dir,
            thing_defs: &defs,
            terrain_defs: &terrain_defs,
            texture_roots: &texture_roots,
            packed_index: packed_index.as_ref(),
            packed_resolver_state: &mut packed_resolver_state,
            width: cli.map_width,
            height: cli.map_height,
        })?;
        if cli.no_window {
            return Ok(());
        }
        let mut app = App::new(render_sprites, cli.screenshot);
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
        let mut sprite_asset =
            resolve_sprite(&data_dir, selected, &texture_roots).with_context(|| {
                format!(
                    "resolving texture for def '{}' path '{}'",
                    selected.def_name, selected.graphic_data.tex_path
                )
            })?;

        let mut resolved_from_packed = false;
        if sprite_asset.used_fallback {
            let can_try_packed = packed_index
                .as_ref()
                .map(|index| index.maybe_contains(&selected.graphic_data.tex_path))
                .unwrap_or(true);
            if can_try_packed && let Some(resolver) = packed_resolver_state.get()? {
                match resolver.resolve(&selected.graphic_data.tex_path) {
                    Ok(Some(hit)) => {
                        sprite_asset.image = hit.image;
                        sprite_asset.source_path = Some(PathBuf::from(hit.source_label));
                        sprite_asset.used_fallback = false;
                        resolved_from_packed = true;
                        info!(
                            "resolved packed texture for '{}' via name '{}'",
                            selected.def_name, hit.matched_name
                        );
                    }
                    Ok(None) => {}
                    Err(err) => {
                        warn!(
                            "packed texture resolve failed for '{}' ({}): {err}",
                            selected.def_name, selected.graphic_data.tex_path
                        );
                    }
                }
            }
        }

        if sprite_asset.used_fallback {
            let can_try_packed = packed_index
                .as_ref()
                .map(|index| index.maybe_contains(&selected.graphic_data.tex_path))
                .unwrap_or(true);
            if can_try_packed {
                if let Some(resolver) = packed_resolver_state.get()? {
                    let probe =
                        resolver.probe_decode_candidates(&selected.graphic_data.tex_path, 8);
                    if probe.attempted > 0 {
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

    let mut app = App::new(render_sprites, cli.screenshot);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct FixtureSceneConfig<'a> {
    data_dir: &'a Path,
    thing_defs: &'a std::collections::HashMap<String, ThingDef>,
    terrain_defs: &'a std::collections::HashMap<String, TerrainDef>,
    texture_roots: &'a [PathBuf],
    packed_index: Option<&'a PackedTextureIndex>,
    packed_resolver_state: &'a mut PackedResolverState,
    width: usize,
    height: usize,
}

fn list_defs(
    defs: &std::collections::HashMap<String, ThingDef>,
    filter: Option<&str>,
    limit: usize,
) {
    let filter_lower = filter.map(|f| f.to_lowercase());
    let mut rows: Vec<_> = defs.values().collect();
    rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));

    let mut shown = 0usize;
    for thing in rows {
        if shown >= limit {
            break;
        }
        if let Some(f) = &filter_lower {
            let name = thing.def_name.to_lowercase();
            let tex = thing.graphic_data.tex_path.to_lowercase();
            if !name.contains(f) && !tex.contains(f) {
                continue;
            }
        }
        println!(
            "{} | texPath={} | class={}",
            thing.def_name,
            thing.graphic_data.tex_path,
            thing
                .graphic_data
                .graphic_class
                .as_deref()
                .unwrap_or("Graphic_Single")
        );
        shown += 1;
    }

    println!("shown {shown} defs (limit {limit})");
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

fn diagnose_textures(data_dir: &std::path::Path, texture_roots: &[PathBuf]) {
    let roots = [
        data_dir.join("Core").join("Textures"),
        data_dir.join("Textures"),
    ];

    for root in roots {
        if !root.exists() {
            println!("missing: {}", root.display());
            continue;
        }
        let png_count = WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .count();
        println!("root: {} | png files: {}", root.display(), png_count);
    }

    for extra in texture_roots {
        let png_count = WalkDir::new(extra)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .count();
        println!("extra root: {} | png files: {}", extra.display(), png_count);
    }

    println!(
        "tip: if counts are near zero, this install likely stores textures in Unity assets; keep using fallback or point --texture-root to an extracted texture dump"
    );
}

fn resolve_texture_with_packed(
    data_dir: &Path,
    tex_path: &str,
    extra_texture_roots: &[PathBuf],
    packed_index: Option<&PackedTextureIndex>,
    packed_resolver_state: &mut PackedResolverState,
) -> Result<SpriteAsset> {
    let mut sprite_asset = resolve_texture_path(data_dir, tex_path, extra_texture_roots)?;
    if !sprite_asset.used_fallback {
        return Ok(sprite_asset);
    }

    let can_try_packed = packed_index
        .map(|index| index.maybe_contains(tex_path))
        .unwrap_or(true);
    if can_try_packed
        && let Some(resolver) = packed_resolver_state.get()?
        && let Ok(Some(hit)) = resolver.resolve(tex_path)
    {
        sprite_asset.image = hit.image;
        sprite_asset.source_path = Some(PathBuf::from(hit.source_label));
        sprite_asset.used_fallback = false;
    }
    Ok(sprite_asset)
}

fn run_terrain_probe(
    data_dir: &Path,
    terrain_defs: &std::collections::HashMap<String, TerrainDef>,
    texture_roots: &[PathBuf],
    packed_index: Option<&PackedTextureIndex>,
    packed_resolver_state: &mut PackedResolverState,
    limit: usize,
) -> Result<()> {
    let mut rows: Vec<_> = terrain_defs.values().collect();
    rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut success = 0usize;
    let mut failed = 0usize;

    for terrain in rows.into_iter().take(limit) {
        let sprite = resolve_texture_with_packed(
            data_dir,
            terrain.texture_path.as_str(),
            texture_roots,
            packed_index,
            packed_resolver_state,
        )?;
        if sprite.used_fallback {
            failed += 1;
            println!(
                "FAIL {:<28} texPath={} source=<fallback>",
                terrain.def_name, terrain.texture_path
            );
        } else {
            success += 1;
            let source = sprite
                .source_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            println!(
                "OK   {:<28} texPath={} source={}",
                terrain.def_name, terrain.texture_path, source
            );
        }
    }
    println!(
        "terrain probe summary: checked={} ok={} failed={}",
        limit, success, failed
    );
    Ok(())
}

fn build_v1_fixture_scene(config: FixtureSceneConfig<'_>) -> Result<Vec<RenderSprite>> {
    let FixtureSceneConfig {
        data_dir,
        thing_defs,
        terrain_defs,
        texture_roots,
        packed_index,
        packed_resolver_state,
        width,
        height,
    } = config;

    let mut terrain_rows: Vec<_> = terrain_defs.values().collect();
    terrain_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));

    let mut chosen_terrain: Vec<(String, image::RgbaImage)> = Vec::new();
    for terrain in terrain_rows {
        let sprite = resolve_texture_with_packed(
            data_dir,
            terrain.texture_path.as_str(),
            texture_roots,
            packed_index,
            packed_resolver_state,
        )?;
        if sprite.used_fallback {
            continue;
        }
        chosen_terrain.push((terrain.def_name.clone(), sprite.image));
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
            let mut sprite_asset = resolve_sprite(data_dir, def, texture_roots)?;
            if sprite_asset.used_fallback {
                sprite_asset = resolve_texture_with_packed(
                    data_dir,
                    def.graphic_data.tex_path.as_str(),
                    texture_roots,
                    packed_index,
                    packed_resolver_state,
                )?;
            }
            if sprite_asset.used_fallback {
                continue;
            }
            thing_choices.push((def.clone(), sprite_asset.image));
        }
    }
    if thing_choices.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable ThingDef");
    }

    let pawn_candidates = [
        "Things/Pawn/Humanlike/Bodies/Naked_Male",
        "Things/Pawn/Humanlike/Bodies/Naked_Female",
        "Things/Pawn/Humanlike/Heads/Male/Average_Normal",
        "Things/Pawn/Humanlike/Heads/Female/Average_Normal",
    ];
    let mut pawn_choices: Vec<(String, image::RgbaImage)> = Vec::new();
    for tex_path in pawn_candidates {
        let sprite = resolve_texture_with_packed(
            data_dir,
            tex_path,
            texture_roots,
            packed_index,
            packed_resolver_state,
        )?;
        if sprite.used_fallback {
            continue;
        }
        pawn_choices.push((tex_path.to_string(), sprite.image));
    }
    if pawn_choices.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable pawn texture");
    }

    let terrain_names = [
        chosen_terrain[0].0.as_str(),
        chosen_terrain[1].0.as_str(),
        chosen_terrain[2].0.as_str(),
    ];
    let thing_names: Vec<String> = thing_choices
        .iter()
        .take(20)
        .map(|(def, _)| def.def_name.clone())
        .collect();
    let pawn_tex: Vec<String> = pawn_choices
        .iter()
        .take(6)
        .map(|(name, _)| name.clone())
        .collect();
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
    let mut pawn_by_tex = std::collections::HashMap::new();
    for (tex_path, image) in pawn_choices {
        pawn_by_tex.insert(tex_path, image);
    }

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
        sprites.push(RenderSprite {
            def_name: format!("Thing::{}", def.def_name),
            image: image.clone(),
            params: SpriteParams {
                world_pos: Vec3::new(
                    thing.cell_x as f32 + 0.5 + draw_offset.x,
                    thing.cell_z as f32 + 0.5 + draw_offset.z,
                    0.2 + draw_offset.y,
                ),
                size: def.graphic_data.draw_size,
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
        let Some(image) = pawn_by_tex.get(&pawn.tex_path) else {
            continue;
        };
        let x_offset = match pawn.facing {
            scene::PawnFacing::East => 0.05,
            scene::PawnFacing::West => -0.05,
            _ => 0.0,
        };
        sprites.push(RenderSprite {
            def_name: format!("Pawn::{}", pawn.label),
            image: image.clone(),
            params: SpriteParams {
                world_pos: Vec3::new(
                    pawn.cell_x as f32 + 0.5 + x_offset,
                    pawn.cell_z as f32 + 0.5,
                    0.6,
                ),
                size: Vec2::new(1.0, 1.0),
                tint: [1.0, 1.0, 1.0, 1.0],
            },
            used_fallback: false,
        });
    }

    info!(
        "v1 fixture scene built: map={}x{} terrain_families={} tiles={} things={} pawns={} drawables={}",
        map.width,
        map.height,
        count_terrain_families(&map),
        map.width * map.height,
        map.things.len(),
        map.pawns.len(),
        sprites.len()
    );
    Ok(sprites)
}

struct App {
    sprites: Vec<RenderSprite>,
    screenshot_path: Option<PathBuf>,
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
    fn new(sprites: Vec<RenderSprite>, screenshot_path: Option<PathBuf>) -> Self {
        Self {
            sprites,
            screenshot_path,
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
        let renderer = pollster::block_on(Renderer::new(window.clone(), sprite_inputs))
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

#[cfg(test)]
mod packed_resolver_state_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::PackedResolverState;

    #[test]
    fn lazy_builder_runs_once_when_result_is_none() {
        let calls = Rc::new(RefCell::new(0usize));
        let calls_for_builder = Rc::clone(&calls);
        let mut state = PackedResolverState::with_builder(
            vec![],
            vec![],
            Box::new(move |_, _| {
                *calls_for_builder.borrow_mut() += 1;
                Ok(None)
            }),
        );

        let _ = state.get().unwrap();
        let _ = state.get().unwrap();
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn disable_prevents_builder_execution() {
        let calls = Rc::new(RefCell::new(0usize));
        let calls_for_builder = Rc::clone(&calls);
        let mut state = PackedResolverState::with_builder(
            vec![],
            vec![],
            Box::new(move |_, _| {
                *calls_for_builder.borrow_mut() += 1;
                Ok(None)
            }),
        );

        state.disable();
        let _ = state.get().unwrap();
        assert_eq!(*calls.borrow(), 0);
    }
}
