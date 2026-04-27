use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glam::Vec2;
use log::info;

use crate::cell::Cell;
use crate::cli::{FixtureCmd, RenderFixturesCmd, ViewArgs};
use crate::runtime::v2::{V2Runtime, V2RuntimeConfig};
use crate::scene::builder::{OwnedSceneDefs, build_scene, compute_pawn_profiles};
use crate::water_assets::WaterAssets;
use crate::world::{build_path_grid, issue_move_intent, tick_world, world_from_fixture};

use super::{CommandAction, DispatchContext, LaunchSpec};

/// RimWorld ships this noise mask as the shared FadeRough / Water alpha
/// variation. Path matches `Verse/TexGame.cs:20`
/// (`ContentFinder<Texture2D>.Get("Other/RoughAlphaAdd")`). The packed
/// resolver matches on basename so the prefix is not load-bearing, but we
/// keep the RimWorld-native path to stay searchable against the decompile.
const ROUGH_ALPHA_ADD_PATH: &str = "Other/RoughAlphaAdd";

pub fn run_fixture(ctx: &mut DispatchContext<'_>, cmd: FixtureCmd) -> Result<CommandAction> {
    build_fixture_action(ctx, cmd)
}

pub fn run_render_fixtures(
    ctx: &mut DispatchContext<'_>,
    cmd: RenderFixturesCmd,
) -> Result<CommandAction> {
    fs::create_dir_all(&cmd.output_dir)
        .with_context(|| format!("creating fixture render dir '{}'", cmd.output_dir.display()))?;
    let mut scenes = fixture_ron_files(&cmd.fixture_dir)?;
    scenes.sort();
    let mut specs = Vec::with_capacity(scenes.len());

    for scene in scenes {
        let stem = scene
            .file_stem()
            .and_then(|stem| stem.to_str())
            .with_context(|| format!("fixture path has no UTF-8 stem: '{}'", scene.display()))?;
        let screenshot = cmd.output_dir.join(format!("{stem}.png"));
        info!(
            "rendering fixture '{}' -> '{}'",
            scene.display(),
            screenshot.display()
        );
        let action = build_fixture_action(
            ctx,
            FixtureCmd {
                scene,
                ticks: cmd.ticks,
                fixed_dt: cmd.fixed_dt,
                view: ViewArgs {
                    screenshot: Some(screenshot),
                    no_window: true,
                    hidden_window: true,
                    viewport_width: cmd.viewport_width,
                    viewport_height: cmd.viewport_height,
                    camera_zoom: cmd.camera_zoom,
                    clear_color: cmd.clear_color,
                },
            },
        )?;
        match action {
            CommandAction::Launch(spec) => specs.push(*spec),
            CommandAction::Done => {}
            CommandAction::LaunchBatch(_) => {
                anyhow::bail!("fixture render unexpectedly returned a nested batch")
            }
        }
    }

    Ok(CommandAction::LaunchBatch(specs))
}

fn build_fixture_action(ctx: &mut DispatchContext<'_>, cmd: FixtureCmd) -> Result<CommandAction> {
    let (should_run_renderer, render_options, hide_window) = crate::cli::render_runtime(&cmd.view);
    let fixture = crate::fixtures::load_fixture(&cmd.scene)?;
    let mut world = world_from_fixture(&fixture);
    let _ = build_path_grid(&world);
    if let Some(first_pawn_id) = world.pawns().first().map(|pawn| pawn.id) {
        let start = {
            let pawn = world.pawns().iter().find(|pawn| pawn.id == first_pawn_id);
            pawn.map(|pawn| Cell::new(pawn.cell_x, pawn.cell_z))
                .unwrap_or(Cell::new(0, 0))
        };
        let _ = issue_move_intent(&mut world, first_pawn_id, start);
        tick_world(&mut world, 0.0);
    }
    let pawn_profiles = compute_pawn_profiles(&ctx.defs, ctx.asset_resolver, &world)?;
    let scene = build_scene(&ctx.defs, &world, &pawn_profiles, &ctx.compose_config)?;
    let noise_image = {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ROUGH_ALPHA_ADD_PATH)
            .with_context(|| format!("resolving noise texture '{ROUGH_ALPHA_ADD_PATH}'"))?;
        if resolved.used_fallback() {
            anyhow::bail!("missing noise texture '{ROUGH_ALPHA_ADD_PATH}'");
        }
        resolved.image
    };
    let blocking_things = world
        .things()
        .iter()
        .filter(|thing| thing.blocks_movement)
        .count();
    let render_state = world.render_state();
    let roofed_cells = render_state.roofs.iter().filter(|roof| roof.roofed).count();
    let thick_roof_cells = render_state.roofs.iter().filter(|roof| roof.thick).count();
    let fogged_cells = render_state.fog.iter().filter(|fogged| **fogged).count();
    let snow_cells = render_state
        .snow_depth
        .iter()
        .filter(|depth| **depth > 0.0)
        .count();
    let max_snow_depth = render_state
        .snow_depth
        .iter()
        .copied()
        .fold(0.0_f32, f32::max);
    let glow_radius_total: f32 = render_state
        .glow_sources
        .iter()
        .map(|source| source.radius)
        .sum();
    let glow_overlight_total: f32 = render_state
        .glow_sources
        .iter()
        .map(|source| source.overlight_radius)
        .sum();
    let glow_anchor_checksum: i32 = render_state
        .glow_sources
        .iter()
        .map(|source| source.cell_x + source.cell_z)
        .sum();
    let glow_color_total: f32 = render_state
        .glow_sources
        .iter()
        .map(|source| source.color.r + source.color.g + source.color.b + source.color.a)
        .sum();
    let sky_glow_total = render_state
        .sky_glow
        .map(|color| color.r + color.g + color.b + color.a)
        .unwrap_or(0.0);
    let shadow_color_total = render_state
        .shadow_color
        .map(|color| color.r + color.g + color.b + color.a)
        .unwrap_or(0.0);

    let camera_focus = fixture
        .camera
        .as_ref()
        .map(|camera| Vec2::new(camera.center_x, camera.center_z))
        .or_else(|| {
            Some(Vec2::new(
                world.width() as f32 * 0.5,
                world.height() as f32 * 0.5,
            ))
        });

    info!(
        "fixture scene built: scene={} map={}x{} terrain={} things={} blocking_things={} pawns={} static={} dynamic={} roofed={} thick_roofs={} fogged={} snow_cells={} max_snow={:.2} day_percent={:?} sky_glow_total={:.2} shadow_color_total={:.2} glow_sources={} glow_radius_total={:.2} glow_overlight_total={:.2} glow_anchor_checksum={} glow_color_total={:.2}",
        cmd.scene.display(),
        world.width(),
        world.height(),
        world.terrain().len(),
        world.things().len(),
        blocking_things,
        world.pawns().len(),
        scene.static_sprites.len(),
        scene.dynamic_sprites.len(),
        roofed_cells,
        thick_roof_cells,
        fogged_cells,
        snow_cells,
        max_snow_depth,
        render_state.day_percent,
        sky_glow_total,
        shadow_color_total,
        render_state.glow_sources.len(),
        glow_radius_total,
        glow_overlight_total,
        glow_anchor_checksum,
        glow_color_total
    );

    // Drop the launch-time dynamic_sprites: V2Runtime owns dynamic-sprite
    // production from the first redraw onward, and the viewer's launch path
    // would otherwise resolve textures for sprites that get immediately
    // overwritten on the next frame.
    let crate::scene::Scene {
        static_sprites,
        edge_sprites,
        static_overlays,
        static_textured_overlays,
        ..
    } = scene;
    let mut runtime = V2Runtime::new(
        world,
        V2RuntimeConfig {
            fixed_dt_seconds: cmd.fixed_dt.unwrap_or(1.0 / 60.0),
            compose_config: ctx.compose_config.clone(),
            scene_defs: Some(OwnedSceneDefs::from_refs(&ctx.defs)),
            pawn_profiles,
        },
    );

    let water_assets = WaterAssets::load(ctx.asset_resolver)?;

    if !should_run_renderer {
        let tick_limit = cmd.ticks.unwrap_or(0);
        for _ in 0..tick_limit {
            runtime.tick_once();
        }
        info!(
            "fixture headless ticks complete: ticks={}",
            runtime.tick_count()
        );
        return Ok(CommandAction::Done);
    }

    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites,
        dynamic_sprites: Vec::new(),
        edge_sprites,
        static_overlays,
        static_textured_overlays,
        noise_image,
        water_assets,
        runtime: Some(runtime),
        runtime_tick_limit: cmd.ticks,
        screenshot: cmd.view.screenshot,
        camera_focus,
        render_options,
        hide_window,
        fixed_step: true,
    })))
}

fn fixture_ron_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries =
        fs::read_dir(dir).with_context(|| format!("reading fixture dir '{}'", dir.display()))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| format!("reading entry in '{}'", dir.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|extension| extension == "ron") {
            out.push(path);
        }
    }
    Ok(out)
}
