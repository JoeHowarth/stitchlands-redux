use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use glam::Vec2;
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
    sprites: Vec<RenderSprite>,
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    renderer_options: RendererOptions,
    hidden_window: bool,
) -> Result<()> {
    let mut app = App::new(
        sprites,
        screenshot_path,
        initial_camera_center,
        renderer_options,
        hidden_window,
    );
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    sprites: Vec<RenderSprite>,
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    screenshot_taken: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_options: RendererOptions,
    hidden_window: bool,
}

impl App {
    fn new(
        sprites: Vec<RenderSprite>,
        screenshot_path: Option<std::path::PathBuf>,
        initial_camera_center: Option<Vec2>,
        renderer_options: RendererOptions,
        hidden_window: bool,
    ) -> Self {
        Self {
            sprites,
            screenshot_path,
            initial_camera_center,
            screenshot_taken: false,
            window: None,
            renderer: None,
            renderer_options,
            hidden_window,
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
        let attrs = if self.hidden_window {
            attrs.with_visible(false)
        } else {
            attrs
        };

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
            self.initial_camera_center,
            self.renderer_options,
        ))
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
                let capture: Option<&Path> = if self.screenshot_taken {
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
