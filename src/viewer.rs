use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use glam::Vec2;
use log::info;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

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
    dynamic_inputs: Vec<SpriteInput>,
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
            dynamic_inputs: Vec::new(),
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
        self.dynamic_inputs = self
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
            .set_dynamic_sprites(self.dynamic_inputs.clone())
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
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                if !self.dynamic_inputs.is_empty()
                    && let Err(err) = renderer.set_dynamic_sprites(self.dynamic_inputs.clone())
                {
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
