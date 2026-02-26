use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use winit::dpi::PhysicalSize;

use crate::renderer::RendererOptions;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    #[command(flatten)]
    pub data: DataArgs,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone)]
pub struct DataArgs {
    #[arg(long)]
    pub rimworld_data: Option<PathBuf>,
    #[arg(long)]
    pub texture_root: Vec<PathBuf>,
    #[arg(long)]
    pub packed_data_root: Vec<PathBuf>,
    #[arg(long)]
    pub packed_index_path: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub rebuild_packed_index: bool,
    #[arg(long, default_value_t = false)]
    pub no_packed_index: bool,
    #[arg(long)]
    pub typetree_registry: Vec<PathBuf>,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub auto_typetree: bool,
    #[arg(long, default_value_t = false)]
    pub allow_fallback: bool,
}

#[derive(Args, Debug, Clone)]
pub struct ViewArgs {
    #[arg(long)]
    pub screenshot: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub no_window: bool,
    #[arg(long, default_value_t = false)]
    pub hidden_window: bool,
    #[arg(long, default_value_t = 1024)]
    pub viewport_width: u32,
    #[arg(long, default_value_t = 1024)]
    pub viewport_height: u32,
    #[arg(long, default_value_t = 6.0)]
    pub camera_zoom: f32,
    #[arg(long, value_parser = parse_clear_color, default_value = "0.05,0.08,0.10,1")]
    pub clear_color: [f64; 4],
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Render(RenderCmd),
    Fixture {
        #[command(subcommand)]
        mode: FixtureCmd,
    },
    Audit(AuditCmd),
    Debug(DebugCmdArgs),
}

#[derive(Args, Debug)]
pub struct DebugCmdArgs {
    #[command(subcommand)]
    pub command: DebugCmd,
}

#[derive(Args, Debug)]
pub struct RenderCmd {
    #[arg(long)]
    pub thingdef: Option<String>,
    #[arg(long)]
    pub image_path: Option<PathBuf>,
    #[arg(long)]
    pub extra_thingdef: Vec<String>,
    #[arg(long)]
    pub export_resolved: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    pub sheet_columns: usize,
    #[arg(long, default_value_t = 1.75)]
    pub sheet_spacing: f32,
    #[arg(long, default_value_t = 0.0)]
    pub cell_x: f32,
    #[arg(long, default_value_t = 0.0)]
    pub cell_z: f32,
    #[arg(long, default_value_t = 1.0)]
    pub scale: f32,
    #[arg(long, value_parser = parse_tint, default_value = "1,1,1,1")]
    pub tint: [f32; 4],
    #[command(flatten)]
    pub view: ViewArgs,
}

#[derive(Subcommand, Debug)]
pub enum FixtureCmd {
    V1(FixtureSceneCmd),
    Pawn(FixtureSceneCmd),
}

#[derive(Args, Debug, Clone)]
pub struct FixtureSceneCmd {
    #[arg(long, default_value_t = 0)]
    pub pawn_fixture_variant: usize,
    #[arg(long)]
    pub dump_pawn_trace: Option<PathBuf>,
    #[arg(long, default_value_t = 40)]
    pub map_width: usize,
    #[arg(long, default_value_t = 40)]
    pub map_height: usize,
    #[command(flatten)]
    pub view: ViewArgs,
}

#[derive(Args, Debug)]
pub struct AuditCmd {
    #[arg(long, default_value_t = 10)]
    pub pawn_count: usize,
    #[arg(long, default_value_t = 0)]
    pub pawn_fixture_variant: usize,
    #[arg(long)]
    pub dump_pawn_trace: Option<PathBuf>,
    #[arg(long, default_value_t = 28)]
    pub map_width: usize,
    #[arg(long, default_value_t = 28)]
    pub map_height: usize,
    #[command(flatten)]
    pub view: ViewArgs,
}

#[derive(Subcommand, Debug)]
pub enum DebugCmd {
    ListDefs {
        #[arg(long)]
        def_filter: Option<String>,
        #[arg(long, default_value_t = 25)]
        list_limit: usize,
    },
    SearchPackedTextures {
        query: String,
        #[arg(long, default_value_t = 20)]
        search_limit: usize,
    },
    DiagnoseTextures,
    ProbeTerrain {
        #[arg(long, default_value_t = 64)]
        terrain_probe_limit: usize,
    },
    ExtractPackedTextures {
        output_dir: PathBuf,
    },
    PackedDecodeProbe {
        #[arg(long, default_value_t = 24)]
        sample_limit: usize,
        #[arg(long, default_value_t = 8)]
        min_attempts: usize,
    },
    ValidateFixture {
        path: PathBuf,
    },
}

pub fn parse_tint(input: &str) -> Result<[f32; 4], String> {
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

pub fn parse_clear_color(input: &str) -> Result<[f64; 4], String> {
    let cleaned = input.replace(',', " ");
    let mut nums = cleaned
        .split_whitespace()
        .map(|v| v.parse::<f64>().map_err(|e| e.to_string()));
    let r = nums.next().ok_or_else(|| "missing r".to_string())??;
    let g = nums.next().ok_or_else(|| "missing g".to_string())??;
    let b = nums.next().ok_or_else(|| "missing b".to_string())??;
    let a = nums.next().transpose()?.unwrap_or(1.0);
    Ok([r, g, b, a])
}

pub fn render_runtime(view: &ViewArgs) -> (bool, RendererOptions, bool) {
    let should_run_renderer = !view.no_window || view.screenshot.is_some();
    let render_options = RendererOptions {
        clear_color: view.clear_color,
        surface_size: Some(PhysicalSize::new(
            view.viewport_width.max(1),
            view.viewport_height.max(1),
        )),
        initial_zoom: Some(view.camera_zoom.max(0.2)),
    };
    let hide_window = view.hidden_window || (view.no_window && view.screenshot.is_some());
    (should_run_renderer, render_options, hide_window)
}
