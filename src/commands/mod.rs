use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use glam::Vec2;

use crate::assets::AssetResolver;
use crate::cli::Command;
use crate::defs::{
    ApparelDef, BeardDefRender, BodyTypeDefRender, HairDefRender, HeadTypeDefRender, TerrainDef,
    ThingDef,
};
use crate::pawn::PawnComposeConfig;
use crate::renderer::RendererOptions;

pub mod common;
mod debug_cmd;
mod fixture_cmd;
mod fixture_v2_cmd;
mod render_cmd;

pub struct DispatchContext<'a> {
    pub data_dir: &'a Path,
    pub thing_defs: &'a HashMap<String, ThingDef>,
    pub terrain_defs: &'a HashMap<String, TerrainDef>,
    pub apparel_defs: &'a HashMap<String, ApparelDef>,
    pub body_type_defs: &'a HashMap<String, BodyTypeDefRender>,
    pub head_type_defs: &'a HashMap<String, HeadTypeDefRender>,
    pub beard_defs: &'a HashMap<String, BeardDefRender>,
    pub hair_defs: &'a HashMap<String, HairDefRender>,
    pub compose_config: PawnComposeConfig,
    pub allow_fallback: bool,
    pub asset_resolver: &'a mut AssetResolver,
}

pub enum CommandAction {
    Done,
    Launch(Box<LaunchSpec>),
}

pub struct LaunchSpec {
    pub static_sprites: Vec<crate::viewer::RenderSprite>,
    pub dynamic_sprites: Vec<crate::viewer::RenderSprite>,
    pub runtime: Option<crate::runtime::v2::V2Runtime>,
    pub screenshot: Option<PathBuf>,
    pub camera_focus: Option<Vec2>,
    pub render_options: RendererOptions,
    pub hide_window: bool,
    pub fixed_step: bool,
}

pub fn dispatch(ctx: &mut DispatchContext<'_>, command: Command) -> Result<CommandAction> {
    match command {
        Command::Debug(debug) => debug_cmd::run(ctx, debug.command),
        Command::Fixture { mode } => fixture_cmd::run_fixture(ctx, mode),
        Command::Audit(audit) => fixture_cmd::run_audit(ctx, audit),
        Command::Render(render) => render_cmd::run(ctx, render),
    }
}
