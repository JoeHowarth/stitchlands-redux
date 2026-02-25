mod assets;
mod defs;
mod rimworld_paths;
mod renderer;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use glam::Vec3;
use log::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::assets::resolve_sprite;
use crate::defs::{load_thing_defs, ThingDef};
use crate::rimworld_paths::resolve_data_dir;
use crate::renderer::{Renderer, SpriteInput, SpriteParams};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(long)]
    rimworld_data: PathBuf,

    #[arg(long)]
    thingdef: Option<String>,

    #[arg(long)]
    extra_thingdef: Vec<String>,

    #[arg(long, default_value_t = false)]
    list_defs: bool,

    #[arg(long)]
    def_filter: Option<String>,

    #[arg(long, default_value_t = 25)]
    list_limit: usize,

    #[arg(long)]
    texture_root: Option<PathBuf>,

    #[arg(long)]
    export_resolved: Option<PathBuf>,

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

    let data_dir = resolve_data_dir(&cli.rimworld_data)
        .with_context(|| format!("resolving rimworld data dir from {}", cli.rimworld_data.display()))?;
    info!("using RimWorld data dir: {}", data_dir.display());

    let defs = load_thing_defs(&data_dir)
        .with_context(|| format!("loading defs from {}", data_dir.display()))?;
    info!("loaded {} thing defs with graphicData", defs.len());

    if cli.list_defs {
        list_defs(&defs, cli.def_filter.as_deref(), cli.list_limit);
        return Ok(());
    }

    let thingdef = cli
        .thingdef
        .as_deref()
        .context("--thingdef is required unless --list-defs is used")?;
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
        let sprite_asset =
            resolve_sprite(&data_dir, selected, cli.texture_root.as_deref()).with_context(|| {
                format!(
                    "resolving texture for def '{}' path '{}'",
                    selected.def_name, selected.graphic_data.tex_path
                )
            })?;

        if sprite_asset.used_fallback {
            warn!(
                "texture missing for '{}' ({}) - using checker fallback",
                selected.def_name, selected.graphic_data.tex_path
            );
            for attempted in sprite_asset.attempted_paths.iter().take(6) {
                info!("attempted: {}", attempted.display());
            }
        }
        if let Some(path) = &sprite_asset.source_path {
            info!("resolved texture: {}", path.display());
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
            index,
            selected.def_name,
            size.x,
            size.y,
            draw_offset.x,
            draw_offset.y,
            draw_offset.z
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

    let mut app = App::new(render_sprites);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn list_defs(defs: &std::collections::HashMap<String, ThingDef>, filter: Option<&str>, limit: usize) {
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

struct App {
    sprites: Vec<RenderSprite>,
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
    fn new(sprites: Vec<RenderSprite>) -> Self {
        Self {
            sprites,
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
            "stitchlands-redux v0 | sprites={} first={} fallback={} | pan: WASD/Arrows zoom: wheel/QE",
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
        ))
        .expect("create renderer");

        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
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
                if let Err(err) = renderer.render() {
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
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}
