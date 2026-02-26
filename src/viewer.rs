use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use glam::Vec2;
use image::RgbaImage;
use log::info;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::interaction::InteractionState;
use crate::renderer::{Renderer, RendererOptions, SpriteInput, SpriteParams};

pub(crate) struct RenderSprite {
    pub(crate) def_name: String,
    pub(crate) image: image::RgbaImage,
    pub(crate) params: SpriteParams,
    pub(crate) used_fallback: bool,
}

pub(crate) fn run_viewer(
    static_sprites: Vec<RenderSprite>,
    dynamic_sprites: Vec<RenderSprite>,
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    renderer_options: RendererOptions,
    hidden_window: bool,
    fixed_step: bool,
) -> Result<()> {
    let mut app = App::new(
        static_sprites,
        dynamic_sprites,
        screenshot_path,
        initial_camera_center,
        renderer_options,
        hidden_window,
        fixed_step,
    );
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    static_sprites: Vec<RenderSprite>,
    dynamic_sprites: Vec<RenderSprite>,
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    screenshot_taken: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_options: RendererOptions,
    hidden_window: bool,
    fixed_step: bool,
    base_dynamic_inputs: Vec<SpriteInput>,
    interaction: InteractionState,
    overlay_image: RgbaImage,
    map_bounds: Option<(usize, usize)>,
    frame_count: u64,
    tick_count: u64,
    step_accumulator: Duration,
    last_step_instant: Option<Instant>,
}

impl App {
    fn new(
        static_sprites: Vec<RenderSprite>,
        dynamic_sprites: Vec<RenderSprite>,
        screenshot_path: Option<std::path::PathBuf>,
        initial_camera_center: Option<Vec2>,
        renderer_options: RendererOptions,
        hidden_window: bool,
        fixed_step: bool,
    ) -> Self {
        Self {
            static_sprites,
            dynamic_sprites,
            screenshot_path,
            initial_camera_center,
            screenshot_taken: false,
            window: None,
            renderer: None,
            renderer_options,
            hidden_window,
            fixed_step,
            base_dynamic_inputs: Vec::new(),
            interaction: InteractionState::default(),
            overlay_image: RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255])
                .expect("1x1 overlay texture"),
            map_bounds: None,
            frame_count: 0,
            tick_count: 0,
            step_accumulator: Duration::ZERO,
            last_step_instant: None,
        }
    }

    fn run_fixed_step(&mut self) {
        let now = Instant::now();
        let previous = self.last_step_instant.unwrap_or(now);
        self.last_step_instant = Some(now);
        self.step_accumulator += now.saturating_duration_since(previous);

        let fixed_dt = Duration::from_secs_f32(1.0 / 60.0);
        while self.step_accumulator >= fixed_dt {
            self.step_accumulator -= fixed_dt;
            self.tick_count += 1;
        }
    }

    fn dynamic_with_overlays(&self) -> Vec<SpriteInput> {
        let mut out = self.base_dynamic_inputs.clone();

        if let Some((x, z)) = self.interaction.hovered_cell {
            out.push(SpriteInput {
                image: self.overlay_image.clone(),
                params: SpriteParams {
                    world_pos: glam::Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.22),
                    size: Vec2::new(1.04, 1.04),
                    tint: [0.20, 0.90, 1.00, 0.28],
                },
            });
        }
        if let Some((x, z)) = self.interaction.selected_cell {
            out.push(SpriteInput {
                image: self.overlay_image.clone(),
                params: SpriteParams {
                    world_pos: glam::Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.21),
                    size: Vec2::new(1.10, 1.10),
                    tint: [1.00, 0.90, 0.20, 0.30],
                },
            });
        }

        out
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let first = self
            .static_sprites
            .first()
            .or_else(|| self.dynamic_sprites.first())
            .expect("at least one sprite exists in app state");
        let fallback_count = self
            .static_sprites
            .iter()
            .chain(self.dynamic_sprites.iter())
            .filter(|s| s.used_fallback)
            .count();
        let total_sprites = self.static_sprites.len() + self.dynamic_sprites.len();
        self.map_bounds = infer_map_bounds(&self.static_sprites);
        let attrs = Window::default_attributes().with_title(format!(
            "stitchlands-redux | sprites={} first={} fallback={} | pan: WASD/Arrows zoom: wheel/QE",
            total_sprites, first.def_name, fallback_count
        ));
        let attrs = if self.hidden_window {
            attrs.with_visible(false)
        } else {
            attrs
        };

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let static_inputs: Vec<SpriteInput> = self
            .static_sprites
            .drain(..)
            .map(|sprite| SpriteInput {
                image: sprite.image,
                params: sprite.params,
            })
            .collect();
        self.base_dynamic_inputs = self
            .dynamic_sprites
            .drain(..)
            .map(|sprite| SpriteInput {
                image: sprite.image,
                params: sprite.params,
            })
            .collect();
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            static_inputs,
            self.initial_camera_center,
            self.renderer_options,
        ))
        .expect("create renderer");
        let mut renderer = renderer;
        renderer
            .set_dynamic_sprites(self.dynamic_with_overlays())
            .expect("set initial dynamic sprites");

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

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size);
                }
            }
            WindowEvent::RedrawRequested => {
                if self.fixed_step {
                    self.run_fixed_step();
                }
                let frame_dynamic = self.dynamic_with_overlays();
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                if let Err(err) = renderer.set_dynamic_sprites(frame_dynamic) {
                    eprintln!("dynamic sprite update error: {err:#}");
                    event_loop.exit();
                    return;
                }
                let capture: Option<&Path> = if self.screenshot_taken {
                    None
                } else {
                    self.screenshot_path.as_deref()
                };
                match renderer.render(capture) {
                    Ok(captured) => {
                        self.frame_count += 1;
                        if self.fixed_step && self.frame_count.is_multiple_of(120) {
                            info!(
                                "v2 runtime counters: frames={} ticks={}",
                                self.frame_count, self.tick_count
                            );
                        }
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
            WindowEvent::CursorMoved { position, .. } => {
                if self.fixed_step {
                    let Some(renderer) = self.renderer.as_ref() else {
                        return;
                    };
                    let world = renderer.screen_to_world(position.x as f32, position.y as f32);
                    let cell = if let Some((w, h)) = self.map_bounds {
                        crate::interaction::world_to_cell_in_bounds(world, w, h)
                    } else {
                        Some(crate::interaction::world_to_cell(world))
                    };
                    if self.interaction.hovered_cell != cell {
                        self.interaction.hovered_cell = cell;
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.fixed_step {
                    self.interaction.selected_cell = self.interaction.hovered_cell;
                    window.request_redraw();
                }
            }
            _ => {
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                if renderer.input(&event) {
                    window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

fn infer_map_bounds(static_sprites: &[RenderSprite]) -> Option<(usize, usize)> {
    let mut max_x = -1i32;
    let mut max_z = -1i32;
    for sprite in static_sprites {
        if !sprite.def_name.starts_with("Terrain::") {
            continue;
        }
        let x = sprite.params.world_pos.x.floor() as i32;
        let z = sprite.params.world_pos.y.floor() as i32;
        max_x = max_x.max(x);
        max_z = max_z.max(z);
    }
    if max_x < 0 || max_z < 0 {
        return None;
    }
    Some(((max_x + 1) as usize, (max_z + 1) as usize))
}
