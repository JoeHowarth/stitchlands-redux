use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use glam::Vec2;
use image::RgbaImage;
use log::info;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::renderer::{
    EdgeSpriteInput, Renderer, RendererOptions, SpriteInput, SpriteInstance, SpriteParams,
    TextureId,
};
use crate::runtime::v2::{
    InteractionOutcome, V2Runtime,
    render_bridge::{PawnNodeTextureCache, compose_dynamic_sprites},
};

pub(crate) struct RenderSprite {
    pub(crate) def_name: String,
    pub(crate) image: image::RgbaImage,
    pub(crate) params: SpriteParams,
    pub(crate) used_fallback: bool,
    pub(crate) pawn_id: Option<usize>,
}

pub(crate) struct ViewerLaunch {
    pub(crate) static_sprites: Vec<RenderSprite>,
    pub(crate) dynamic_sprites: Vec<RenderSprite>,
    pub(crate) edge_sprites: Vec<EdgeSpriteInput>,
    pub(crate) noise_image: RgbaImage,
    pub(crate) screenshot_path: Option<std::path::PathBuf>,
    pub(crate) initial_camera_center: Option<Vec2>,
    pub(crate) renderer_options: RendererOptions,
    pub(crate) hidden_window: bool,
    pub(crate) fixed_step: bool,
    pub(crate) runtime: Option<V2Runtime>,
    pub(crate) runtime_tick_limit: Option<u64>,
}

pub(crate) fn run_viewer(launch: ViewerLaunch) -> Result<()> {
    let mut app = App::new(launch);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    static_sprites: Vec<RenderSprite>,
    dynamic_sprites: Vec<RenderSprite>,
    edge_sprites: Vec<EdgeSpriteInput>,
    noise_image: RgbaImage,
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    screenshot_taken: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_options: RendererOptions,
    hidden_window: bool,
    fixed_step: bool,
    base_dynamic_inputs: Vec<SpriteInstance>,
    pawn_node_textures: PawnNodeTextureCache,
    overlay_image: RgbaImage,
    overlay_texture_id: Option<TextureId>,
    map_bounds: Option<(usize, usize)>,
    runtime: Option<V2Runtime>,
    runtime_tick_limit: Option<u64>,
    runtime_finished: bool,
}

impl App {
    fn new(launch: ViewerLaunch) -> Self {
        Self {
            static_sprites: launch.static_sprites,
            dynamic_sprites: launch.dynamic_sprites,
            edge_sprites: launch.edge_sprites,
            noise_image: launch.noise_image,
            screenshot_path: launch.screenshot_path,
            initial_camera_center: launch.initial_camera_center,
            screenshot_taken: false,
            window: None,
            renderer: None,
            renderer_options: launch.renderer_options,
            hidden_window: launch.hidden_window,
            fixed_step: launch.fixed_step,
            base_dynamic_inputs: Vec::new(),
            pawn_node_textures: HashMap::new(),
            overlay_image: RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255])
                .expect("1x1 overlay texture"),
            overlay_texture_id: None,
            map_bounds: None,
            runtime: launch.runtime,
            runtime_tick_limit: launch.runtime_tick_limit,
            runtime_finished: false,
        }
    }

    fn dynamic_with_overlays(&self) -> Vec<SpriteInstance> {
        if let Some(runtime) = &self.runtime
            && let Some(overlay_texture_id) = self.overlay_texture_id
        {
            let frame = runtime.frame_output();
            compose_dynamic_sprites(
                &self.base_dynamic_inputs,
                &self.pawn_node_textures,
                overlay_texture_id,
                &frame,
            )
        } else {
            self.base_dynamic_inputs.clone()
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
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            static_inputs,
            self.noise_image.clone(),
            self.initial_camera_center,
            self.renderer_options,
        ))
        .expect("create renderer");
        let mut renderer = renderer;
        let edge_inputs: Vec<EdgeSpriteInput> = self.edge_sprites.drain(..).collect();
        renderer
            .set_static_edge_sprites(edge_inputs)
            .expect("set static edge sprites");
        self.base_dynamic_inputs.clear();
        self.pawn_node_textures.clear();
        self.overlay_texture_id = Some(renderer.register_texture(self.overlay_image.clone()));
        for sprite in self.dynamic_sprites.drain(..) {
            let texture_id = renderer.register_texture(sprite.image);
            if let Some(pawn_id) = sprite.pawn_id
                && let Some(node_id) = parse_pawn_node_id(&sprite.def_name)
            {
                self.pawn_node_textures
                    .entry(pawn_id)
                    .or_default()
                    .insert(node_id.to_string(), texture_id);
                continue;
            }

            self.base_dynamic_inputs.push(SpriteInstance {
                texture_id,
                params: sprite.params,
            });
        }
        renderer
            .set_dynamic_instances(self.dynamic_with_overlays())
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
        let Some(expected_window_id) = self.window.as_ref().map(|window| window.id()) else {
            return;
        };
        if expected_window_id != window_id {
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
                if self.fixed_step
                    && let Some(runtime) = self.runtime.as_mut()
                {
                    if let Some(limit) = self.runtime_tick_limit {
                        if runtime.tick_count() < limit {
                            runtime.tick_once();
                        }
                    } else {
                        runtime.run_fixed_step();
                    }
                }
                let frame_dynamic = self.dynamic_with_overlays();
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                if let Err(err) = renderer.set_dynamic_instances(frame_dynamic) {
                    eprintln!("dynamic sprite update error: {err:#}");
                    event_loop.exit();
                    return;
                }
                let reached_tick_limit = self
                    .runtime
                    .as_ref()
                    .and_then(|runtime| {
                        self.runtime_tick_limit
                            .map(|limit| runtime.tick_count() >= limit)
                    })
                    .unwrap_or(false);
                let capture: Option<&Path> = if self.screenshot_taken {
                    None
                } else if self.runtime_tick_limit.is_some() {
                    if reached_tick_limit {
                        self.screenshot_path.as_deref()
                    } else {
                        None
                    }
                } else {
                    self.screenshot_path.as_deref()
                };
                match renderer.render(capture) {
                    Ok(captured) => {
                        if let Some(runtime) = self.runtime.as_mut() {
                            runtime.bump_frame_count();
                            if self.fixed_step && runtime.frame_count().is_multiple_of(120) {
                                info!(
                                    "v2 runtime counters: frames={} ticks={}",
                                    runtime.frame_count(),
                                    runtime.tick_count()
                                );
                            }
                        }
                        if captured {
                            self.screenshot_taken = true;
                        }
                        if reached_tick_limit {
                            if let Some(runtime) = self.runtime.as_ref()
                                && !self.runtime_finished
                            {
                                self.runtime_finished = true;
                                info!(
                                    "v2 runtime complete: frames={} ticks={}",
                                    runtime.frame_count(),
                                    runtime.tick_count()
                                );
                            }
                            if self.screenshot_path.is_none() || self.screenshot_taken {
                                event_loop.exit();
                            }
                        } else if captured {
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
                    let bounds = self
                        .runtime
                        .as_ref()
                        .map(|r| r.map_bounds())
                        .or(self.map_bounds);
                    let cell = if let Some((w, h)) = bounds {
                        crate::interaction::world_to_cell_in_bounds(world, w, h)
                    } else {
                        Some(crate::interaction::world_to_cell(world))
                    };
                    let hovered_changed = self
                        .runtime
                        .as_mut()
                        .map(|runtime| runtime.on_cursor_cell(cell))
                        .unwrap_or(false);
                    if hovered_changed && let Some(window) = self.window.as_ref() {
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
                    if let Some(runtime) = self.runtime.as_mut() {
                        match runtime.on_left_click() {
                            InteractionOutcome::SelectedPawn { pawn_id, cell } => {
                                info!(
                                    "selected pawn id={} at cell=({}, {})",
                                    pawn_id, cell.x, cell.z
                                );
                            }
                            InteractionOutcome::IssuedMove { pawn_id, dest } => {
                                info!(
                                    "issued move pawn id={} to cell=({}, {})",
                                    pawn_id, dest.x, dest.z
                                );
                                if let Some(is_idle) = runtime.selected_pawn_idle()
                                    && is_idle
                                {
                                    info!("selected pawn id={} remains idle", pawn_id);
                                }
                            }
                            InteractionOutcome::NoOp
                            | InteractionOutcome::SelectedCell(_)
                            | InteractionOutcome::ClearedSelection => {}
                        }
                    }
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if self.fixed_step {
                    if let Some(runtime) = self.runtime.as_mut() {
                        let _ = runtime.on_right_click();
                    }
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                }
            }
            _ => {
                if let WindowEvent::KeyboardInput { event, .. } = &event
                    && event.state == ElementState::Pressed
                    && let PhysicalKey::Code(KeyCode::Escape) = event.physical_key
                {
                    if let Some(runtime) = self.runtime.as_mut() {
                        let _ = runtime.on_escape();
                    }
                    if let Some(window) = self.window.as_ref() {
                        window.request_redraw();
                    }
                    return;
                }
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                if renderer.input(&event)
                    && let Some(window) = self.window.as_ref()
                {
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

fn parse_pawn_node_id(def_name: &str) -> Option<&str> {
    def_name.strip_prefix("PawnNode::")
}
