mod app_context;
mod assets;
mod cell;
mod cli;
mod commands;
mod defs;
mod fixtures;
mod interaction;
mod linking;
mod path;
mod pawn;
mod renderer;
mod runtime;
mod viewer;
mod water_assets;
mod world;

use anyhow::Result;
use clap::Parser;

use crate::app_context::AppContext;
use crate::cli::Cli;
use crate::defs::HumanlikeRenderTreeLayers;
use crate::pawn::PawnComposeConfig;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();

    let ctx = AppContext::load(&cli.data, compose_config_from_humanlike_layers)?;
    let data_dir = ctx.data_dir.clone();
    let compose_config = ctx.compose_config.clone();
    let mut asset_resolver = ctx.asset_resolver;

    let defs = crate::commands::DefSet {
        thing_defs: &ctx.thing_defs,
        terrain_defs: &ctx.terrain_defs,
        apparel_defs: &ctx.apparel_defs,
        body_type_defs: &ctx.body_type_defs,
        head_type_defs: &ctx.head_type_defs,
        beard_defs: &ctx.beard_defs,
        hair_defs: &ctx.hair_defs,
    };

    let mut dispatch = crate::commands::DispatchContext {
        data_dir: &data_dir,
        defs,
        compose_config,
        asset_resolver: &mut asset_resolver,
    };

    match crate::commands::dispatch(&mut dispatch, cli.command)? {
        crate::commands::CommandAction::Done => Ok(()),
        crate::commands::CommandAction::Launch(spec) => {
            crate::viewer::run_viewer(crate::viewer::ViewerLaunch {
                static_sprites: spec.static_sprites,
                dynamic_sprites: spec.dynamic_sprites,
                edge_sprites: spec.edge_sprites,
                static_overlays: spec.static_overlays,
                noise_image: spec.noise_image,
                water_assets: spec.water_assets,
                screenshot_path: spec.screenshot,
                initial_camera_center: spec.camera_focus,
                renderer_options: spec.render_options,
                hidden_window: spec.hide_window,
                fixed_step: spec.fixed_step,
                runtime: spec.runtime,
                runtime_tick_limit: spec.runtime_tick_limit,
            })
        }
        crate::commands::CommandAction::LaunchBatch(specs) => {
            let launches = specs
                .into_iter()
                .map(|spec| crate::viewer::ViewerLaunch {
                    static_sprites: spec.static_sprites,
                    dynamic_sprites: spec.dynamic_sprites,
                    edge_sprites: spec.edge_sprites,
                    static_overlays: spec.static_overlays,
                    noise_image: spec.noise_image,
                    water_assets: spec.water_assets,
                    screenshot_path: spec.screenshot,
                    initial_camera_center: spec.camera_focus,
                    renderer_options: spec.render_options,
                    hidden_window: spec.hide_window,
                    fixed_step: spec.fixed_step,
                    runtime: spec.runtime,
                    runtime_tick_limit: spec.runtime_tick_limit,
                })
                .collect();
            crate::viewer::run_viewer_batch(launches)
        }
    }
}

fn compose_config_from_humanlike_layers(layers: HumanlikeRenderTreeLayers) -> PawnComposeConfig {
    let mut out = PawnComposeConfig::default();
    let pawn_base_z = -0.6;
    out.layering.body_z =
        pawn_base_z + crate::pawn::workers::layer_to_z_delta(layers.body_base_layer);
    out.layering.head_z =
        pawn_base_z + crate::pawn::workers::layer_to_z_delta(layers.head_base_layer);
    out.layering.beard_z =
        pawn_base_z + crate::pawn::workers::layer_to_z_delta(layers.beard_base_layer);
    out.layering.hair_z =
        pawn_base_z + crate::pawn::workers::layer_to_z_delta(layers.hair_base_layer);
    out.layering.apparel_body_base_z =
        pawn_base_z + crate::pawn::workers::layer_to_z_delta(layers.apparel_body_base_layer);
    out.layering.apparel_head_base_z =
        pawn_base_z + crate::pawn::workers::layer_to_z_delta(layers.apparel_head_base_layer);
    out.layering.apparel_step_z = crate::pawn::workers::layer_to_z_delta(1.0);
    out
}
