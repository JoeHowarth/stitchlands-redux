mod assets;
mod defs;
mod renderer;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use glam::{Vec2, Vec3};
use log::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::assets::resolve_sprite;
use crate::defs::{load_thing_defs, ThingDef};
use crate::renderer::{Renderer, SpriteParams};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    #[arg(long)]
    rimworld_data: PathBuf,

    #[arg(long)]
    thingdef: String,

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
    env_logger::init();
    let cli = Cli::parse();

    let defs = load_thing_defs(&cli.rimworld_data)
        .with_context(|| format!("loading defs from {}", cli.rimworld_data.display()))?;
    info!("loaded {} thing defs with graphicData", defs.len());

    let thing = defs
        .get(&cli.thingdef)
        .cloned()
        .with_context(|| format!("thingdef '{}' not found", cli.thingdef))?;
    info!("selected def: {}", thing.def_name);

    let sprite_asset = resolve_sprite(&cli.rimworld_data, &thing).with_context(|| {
        format!(
            "resolving texture for def '{}' path '{}'",
            thing.def_name, thing.graphic_data.tex_path
        )
    })?;

    if sprite_asset.used_fallback {
        warn!(
            "texture missing for '{}' ({}) - using checker fallback",
            thing.def_name, thing.graphic_data.tex_path
        );
    }
    if let Some(path) = &sprite_asset.source_path {
        info!("resolved texture: {}", path.display());
    }

    let size = thing.graphic_data.draw_size * cli.scale;
    let draw_offset = thing.graphic_data.draw_offset;
    let world_pos = Vec3::new(
        cli.cell_x + 0.5 + draw_offset.x,
        draw_offset.y,
        cli.cell_z + 0.5 + draw_offset.z,
    );
    let tint = [
        cli.tint[0] * thing.graphic_data.color.r,
        cli.tint[1] * thing.graphic_data.color.g,
        cli.tint[2] * thing.graphic_data.color.b,
        cli.tint[3] * thing.graphic_data.color.a,
    ];

    info!(
        "sprite params -> size=({:.2}, {:.2}) offset=({:.2}, {:.2}, {:.2})",
        size.x, size.y, draw_offset.x, draw_offset.y, draw_offset.z
    );

    let mut app = App::new(thing, sprite_asset.image, world_pos, size, tint, sprite_asset.used_fallback);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    thing: ThingDef,
    image: image::RgbaImage,
    sprite_world_pos: Vec3,
    sprite_size: Vec2,
    tint: [f32; 4],
    used_fallback: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}

impl App {
    fn new(
        thing: ThingDef,
        image: image::RgbaImage,
        sprite_world_pos: Vec3,
        sprite_size: Vec2,
        tint: [f32; 4],
        used_fallback: bool,
    ) -> Self {
        Self {
            thing,
            image,
            sprite_world_pos,
            sprite_size,
            tint,
            used_fallback,
            window: None,
            renderer: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes().with_title(format!(
            "stitchlands-redux v0 | {} | fallback={} | pan: WASD/Arrows zoom: wheel/QE",
            self.thing.def_name, self.used_fallback
        ));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            &self.image,
            SpriteParams {
                world_pos: self.sprite_world_pos,
                size: self.sprite_size,
                tint: self.tint,
            },
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
