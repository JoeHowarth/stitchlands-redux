use std::path::{Path, PathBuf};

use anyhow::Result;
use glam::Vec2;

use crate::assets::AssetResolver;
use crate::cli::Command;
use crate::pawn::PawnComposeConfig;
use crate::renderer::RendererOptions;
use crate::water_assets::WaterAssets;

pub mod common;
mod debug_cmd;
mod fixture_cmd;
mod glow_grid;
mod lighting_overlay;
mod linking_sprites;
mod overlays;
mod render_cmd;
mod shadow_overlay;
mod sky_shadow;

pub use common::DefSet;

pub struct DispatchContext<'a> {
    pub data_dir: &'a Path,
    pub defs: DefSet<'a>,
    pub compose_config: PawnComposeConfig,
    pub asset_resolver: &'a mut AssetResolver,
}

pub enum CommandAction {
    Done,
    Launch(Box<LaunchSpec>),
    LaunchBatch(Vec<LaunchSpec>),
}

pub struct LaunchSpec {
    pub static_sprites: Vec<crate::viewer::RenderSprite>,
    pub dynamic_sprites: Vec<crate::viewer::RenderSprite>,
    pub edge_sprites: Vec<crate::renderer::EdgeSpriteInput>,
    pub static_overlays: Vec<crate::renderer::ColoredMeshInput>,
    pub noise_image: image::RgbaImage,
    pub water_assets: WaterAssets,
    pub runtime: Option<crate::runtime::v2::V2Runtime>,
    pub runtime_tick_limit: Option<u64>,
    pub screenshot: Option<PathBuf>,
    pub camera_focus: Option<Vec2>,
    pub render_options: RendererOptions,
    pub hide_window: bool,
    pub fixed_step: bool,
}

pub fn dispatch(ctx: &mut DispatchContext<'_>, command: Command) -> Result<CommandAction> {
    match command {
        Command::Debug(debug) => debug_cmd::run(ctx, debug.command),
        Command::Fixture(cmd) => fixture_cmd::run_fixture(ctx, cmd),
        Command::RenderFixtures(cmd) => fixture_cmd::run_render_fixtures(ctx, cmd),
        Command::Render(render) => render_cmd::run(ctx, render),
    }
}
