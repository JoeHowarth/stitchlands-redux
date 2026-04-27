use std::collections::VecDeque;
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

use crate::assets::AssetResolver;
use crate::renderer::{Renderer, RendererOptions};
use crate::runtime::v2::{InteractionOutcome, V2Runtime, apply_interaction_overlays};
use crate::scene::{
    ColoredMeshInput, EdgeSpriteInput, SceneSprite, SpriteInput, SpriteRecord, TextureHandle,
    TexturedMeshInput,
};
use crate::water_assets::WaterAssets;

pub(crate) struct ViewerLaunch {
    pub(crate) static_sprites: Vec<SceneSprite>,
    pub(crate) dynamic_sprites: Vec<SceneSprite>,
    pub(crate) edge_sprites: Vec<EdgeSpriteInput>,
    pub(crate) static_overlays: Vec<ColoredMeshInput>,
    pub(crate) static_textured_overlays: Vec<TexturedMeshInput>,
    pub(crate) noise_image: RgbaImage,
    pub(crate) water_assets: WaterAssets,
    pub(crate) screenshot_path: Option<std::path::PathBuf>,
    pub(crate) initial_camera_center: Option<Vec2>,
    pub(crate) renderer_options: RendererOptions,
    pub(crate) hidden_window: bool,
    pub(crate) fixed_step: bool,
    pub(crate) runtime: Option<V2Runtime>,
    pub(crate) runtime_tick_limit: Option<u64>,
}

pub(crate) fn run_viewer(asset_resolver: AssetResolver, launch: ViewerLaunch) -> Result<()> {
    let mut app = App::new(asset_resolver, launch, VecDeque::new());
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

pub(crate) fn run_viewer_batch(
    asset_resolver: AssetResolver,
    launches: Vec<ViewerLaunch>,
) -> Result<()> {
    let mut launches = VecDeque::from(launches);
    let Some(first) = launches.pop_front() else {
        return Ok(());
    };
    let mut app = App::new(asset_resolver, first, launches);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    asset_resolver: AssetResolver,
    static_sprites: Vec<SceneSprite>,
    dynamic_sprites: Vec<SceneSprite>,
    edge_sprites: Vec<EdgeSpriteInput>,
    static_overlays: Vec<ColoredMeshInput>,
    static_textured_overlays: Vec<TexturedMeshInput>,
    noise_image: RgbaImage,
    water_assets: Option<WaterAssets>,
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    screenshot_taken: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_options: RendererOptions,
    hidden_window: bool,
    fixed_step: bool,
    base_dynamic_inputs: Vec<SpriteRecord>,
    overlay_image: RgbaImage,
    overlay_texture_id: Option<TextureHandle>,
    map_bounds: Option<(usize, usize)>,
    runtime: Option<V2Runtime>,
    runtime_tick_limit: Option<u64>,
    runtime_finished: bool,
    pending_launches: VecDeque<ViewerLaunch>,
}

impl App {
    fn new(
        asset_resolver: AssetResolver,
        launch: ViewerLaunch,
        pending_launches: VecDeque<ViewerLaunch>,
    ) -> Self {
        Self {
            asset_resolver,
            static_sprites: launch.static_sprites,
            dynamic_sprites: launch.dynamic_sprites,
            edge_sprites: launch.edge_sprites,
            static_overlays: launch.static_overlays,
            static_textured_overlays: launch.static_textured_overlays,
            noise_image: launch.noise_image,
            water_assets: Some(launch.water_assets),
            screenshot_path: launch.screenshot_path,
            initial_camera_center: launch.initial_camera_center,
            screenshot_taken: false,
            window: None,
            renderer: None,
            renderer_options: launch.renderer_options,
            hidden_window: launch.hidden_window,
            fixed_step: launch.fixed_step,
            base_dynamic_inputs: Vec::new(),
            overlay_image: RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255])
                .expect("1x1 overlay texture"),
            overlay_texture_id: None,
            map_bounds: None,
            runtime: launch.runtime,
            runtime_tick_limit: launch.runtime_tick_limit,
            runtime_finished: false,
            pending_launches,
        }
    }

    fn load_launch(&mut self, launch: ViewerLaunch) {
        self.static_sprites = launch.static_sprites;
        self.dynamic_sprites = launch.dynamic_sprites;
        self.edge_sprites = launch.edge_sprites;
        self.static_overlays = launch.static_overlays;
        self.static_textured_overlays = launch.static_textured_overlays;
        self.noise_image = launch.noise_image;
        self.water_assets = Some(launch.water_assets);
        self.screenshot_path = launch.screenshot_path;
        self.initial_camera_center = launch.initial_camera_center;
        self.screenshot_taken = false;
        self.renderer_options = launch.renderer_options;
        self.hidden_window = launch.hidden_window;
        self.fixed_step = launch.fixed_step;
        self.base_dynamic_inputs.clear();
        self.overlay_image =
            RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255]).expect("1x1 overlay texture");
        self.overlay_texture_id = None;
        self.map_bounds = None;
        self.runtime = launch.runtime;
        self.runtime_tick_limit = launch.runtime_tick_limit;
        self.runtime_finished = false;
    }

    fn dynamic_with_overlays(&self) -> Vec<SpriteRecord> {
        if let Some(runtime) = &self.runtime
            && let Some(overlay_texture_id) = self.overlay_texture_id
        {
            let frame = runtime.frame_output();
            apply_interaction_overlays(&self.base_dynamic_inputs, overlay_texture_id, &frame)
        } else {
            self.base_dynamic_inputs.clone()
        }
    }
    fn prepare_renderer(&mut self, event_loop: &ActiveEventLoop) {
        let first = self
            .static_sprites
            .first()
            .or_else(|| self.dynamic_sprites.first())
            .expect("at least one sprite exists in app state");
        let total_sprites = self.static_sprites.len() + self.dynamic_sprites.len();
        self.map_bounds = infer_map_bounds(&self.static_sprites);
        let title = format!(
            "stitchlands-redux | sprites={} first={} | pan: WASD/Arrows zoom: wheel/QE",
            total_sprites, first.def_name
        );
        let window = if let Some(window) = self.window.as_ref() {
            window.set_title(&title);
            window.clone()
        } else {
            let attrs = Window::default_attributes().with_title(title);
            let attrs = if self.hidden_window {
                attrs.with_visible(false)
            } else {
                attrs
            };
            Arc::new(event_loop.create_window(attrs).expect("create window"))
        };
        let static_inputs: Vec<SpriteInput> = self
            .static_sprites
            .drain(..)
            .map(|sprite| SpriteInput {
                texture: sprite.texture,
                params: sprite.params,
                material: sprite.material,
            })
            .collect();
        let water_assets = self
            .water_assets
            .take()
            .expect("water assets already consumed");
        let renderer = pollster::block_on(Renderer::new(
            window.clone(),
            &mut self.asset_resolver,
            static_inputs,
            self.noise_image.clone(),
            water_assets,
            self.initial_camera_center,
            self.renderer_options,
        ))
        .expect("create renderer");
        let mut renderer = renderer;
        let edge_inputs: Vec<EdgeSpriteInput> = self.edge_sprites.drain(..).collect();
        renderer
            .set_static_edge_sprites(&mut self.asset_resolver, edge_inputs)
            .expect("set static edge sprites");
        let static_overlays: Vec<ColoredMeshInput> = self.static_overlays.drain(..).collect();
        renderer
            .set_static_overlays(static_overlays)
            .expect("set static overlays");
        let static_textured_overlays: Vec<TexturedMeshInput> =
            self.static_textured_overlays.drain(..).collect();
        renderer
            .set_static_textured_overlays(&mut self.asset_resolver, static_textured_overlays)
            .expect("set static textured overlays");
        self.overlay_texture_id = Some(renderer.register_texture(self.overlay_image.clone()));
        let dynamic_sprites: Vec<SceneSprite> = self.dynamic_sprites.drain(..).collect();
        populate_dynamic_records(
            &mut self.base_dynamic_inputs,
            &mut renderer,
            &mut self.asset_resolver,
            dynamic_sprites,
        );
        renderer
            .set_dynamic_instances(self.dynamic_with_overlays())
            .expect("set initial dynamic sprites");

        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn finish_or_load_next(&mut self, event_loop: &ActiveEventLoop) {
        let Some(next) = self.pending_launches.pop_front() else {
            event_loop.exit();
            return;
        };
        self.load_launch(next);
        self.prepare_renderer(event_loop);
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.prepare_renderer(event_loop);
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
                if let Some(runtime) = self.runtime.as_ref() {
                    let scene = match runtime.build_scene() {
                        Ok(scene) => scene,
                        Err(err) => {
                            eprintln!("runtime scene build error: {err:#}");
                            event_loop.exit();
                            return;
                        }
                    };
                    let Some(renderer) = self.renderer.as_mut() else {
                        return;
                    };
                    populate_dynamic_records(
                        &mut self.base_dynamic_inputs,
                        renderer,
                        &mut self.asset_resolver,
                        scene.dynamic_sprites,
                    );
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
                                self.finish_or_load_next(event_loop);
                            }
                        } else if captured {
                            self.finish_or_load_next(event_loop);
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

/// Resolve every scene sprite to a renderer-ready `SpriteRecord` and write the
/// result into `out`, replacing any previous contents. Free function so callers
/// can pass split borrows of fields on the parent `App` without fighting the
/// borrow checker.
fn populate_dynamic_records(
    out: &mut Vec<SpriteRecord>,
    renderer: &mut Renderer,
    asset_resolver: &mut AssetResolver,
    sprites: Vec<SceneSprite>,
) {
    out.clear();
    for sprite in sprites {
        let texture_id = renderer
            .resolve_texture(asset_resolver, &sprite.texture)
            .expect("resolve dynamic sprite texture");
        out.push(SpriteRecord {
            texture: texture_id,
            params: sprite.params,
            material: sprite.material,
        });
    }
}

fn infer_map_bounds(static_sprites: &[SceneSprite]) -> Option<(usize, usize)> {
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
