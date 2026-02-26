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
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::interaction::InteractionState;
use crate::renderer::{Renderer, RendererOptions, SpriteInput, SpriteParams};

pub(crate) struct RenderSprite {
    pub(crate) def_name: String,
    pub(crate) image: image::RgbaImage,
    pub(crate) params: SpriteParams,
    pub(crate) used_fallback: bool,
    pub(crate) pawn_id: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePawnHint {
    pub(crate) id: usize,
    pub(crate) cell_x: i32,
    pub(crate) cell_z: i32,
    pub(crate) world_pos: Vec2,
    pub(crate) move_speed_cells_per_sec: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeHints {
    pub(crate) map_width: usize,
    pub(crate) map_height: usize,
    pub(crate) blocking_cells: Vec<(i32, i32)>,
    pub(crate) pawns: Vec<RuntimePawnHint>,
}

pub(crate) struct ViewerLaunch {
    pub(crate) static_sprites: Vec<RenderSprite>,
    pub(crate) dynamic_sprites: Vec<RenderSprite>,
    pub(crate) screenshot_path: Option<std::path::PathBuf>,
    pub(crate) initial_camera_center: Option<Vec2>,
    pub(crate) renderer_options: RendererOptions,
    pub(crate) hidden_window: bool,
    pub(crate) fixed_step: bool,
    pub(crate) runtime_hints: Option<RuntimeHints>,
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
    screenshot_path: Option<std::path::PathBuf>,
    initial_camera_center: Option<Vec2>,
    screenshot_taken: bool,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    renderer_options: RendererOptions,
    hidden_window: bool,
    fixed_step: bool,
    base_dynamic_inputs: Vec<SpriteInput>,
    base_dynamic_pawn_ids: Vec<Option<usize>>,
    interaction: InteractionState,
    overlay_image: RgbaImage,
    map_bounds: Option<(usize, usize)>,
    runtime_state: Option<RuntimeState>,
    frame_count: u64,
    tick_count: u64,
    step_accumulator: Duration,
    last_step_instant: Option<Instant>,
}

#[derive(Debug, Clone)]
struct RuntimePawnState {
    id: usize,
    cell_x: i32,
    cell_z: i32,
    initial_world_pos: Vec2,
    world_pos: Vec2,
    move_speed_cells_per_sec: f32,
    path_cells: Vec<(i32, i32)>,
    path_index: usize,
}

#[derive(Debug, Clone)]
struct RuntimeState {
    map_width: usize,
    map_height: usize,
    blocking_cells: Vec<(i32, i32)>,
    pawns: Vec<RuntimePawnState>,
    selected_pawn_id: Option<usize>,
}

impl App {
    fn new(launch: ViewerLaunch) -> Self {
        let runtime_state = launch.runtime_hints.map(|hints| RuntimeState {
            map_width: hints.map_width,
            map_height: hints.map_height,
            blocking_cells: hints.blocking_cells,
            pawns: hints
                .pawns
                .into_iter()
                .map(|pawn| RuntimePawnState {
                    id: pawn.id,
                    cell_x: pawn.cell_x,
                    cell_z: pawn.cell_z,
                    initial_world_pos: pawn.world_pos,
                    world_pos: pawn.world_pos,
                    move_speed_cells_per_sec: pawn.move_speed_cells_per_sec,
                    path_cells: Vec::new(),
                    path_index: 0,
                })
                .collect(),
            selected_pawn_id: None,
        });
        Self {
            static_sprites: launch.static_sprites,
            dynamic_sprites: launch.dynamic_sprites,
            screenshot_path: launch.screenshot_path,
            initial_camera_center: launch.initial_camera_center,
            screenshot_taken: false,
            window: None,
            renderer: None,
            renderer_options: launch.renderer_options,
            hidden_window: launch.hidden_window,
            fixed_step: launch.fixed_step,
            base_dynamic_inputs: Vec::new(),
            base_dynamic_pawn_ids: Vec::new(),
            interaction: InteractionState::default(),
            overlay_image: RgbaImage::from_raw(1, 1, vec![255, 255, 255, 255])
                .expect("1x1 overlay texture"),
            map_bounds: None,
            runtime_state,
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
            self.tick_runtime(fixed_dt.as_secs_f32());
        }
    }

    fn tick_runtime(&mut self, dt_seconds: f32) {
        let Some(runtime) = self.runtime_state.as_mut() else {
            return;
        };
        for pawn in &mut runtime.pawns {
            if pawn.path_index >= pawn.path_cells.len() {
                continue;
            }
            let target_cell = pawn.path_cells[pawn.path_index];
            let target = Vec2::new(target_cell.0 as f32 + 0.5, target_cell.1 as f32 + 0.5);
            let to_target = target - pawn.world_pos;
            let distance = to_target.length();
            let max_step = pawn.move_speed_cells_per_sec.max(0.1) * dt_seconds.max(0.0);
            if distance <= max_step {
                pawn.world_pos = target;
                pawn.cell_x = target_cell.0;
                pawn.cell_z = target_cell.1;
                pawn.path_index += 1;
            } else if distance > 0.0 {
                pawn.world_pos += to_target / distance * max_step;
            }
        }
    }

    fn issue_pawn_move(&mut self, pawn_id: usize, dest: (i32, i32)) -> bool {
        let Some(runtime) = self.runtime_state.as_mut() else {
            return false;
        };
        let Some(pawn_index) = runtime.pawns.iter().position(|pawn| pawn.id == pawn_id) else {
            return false;
        };
        let start = (
            runtime.pawns[pawn_index].cell_x,
            runtime.pawns[pawn_index].cell_z,
        );
        let mut grid = crate::path::PathGrid::new(runtime.map_width, runtime.map_height);
        for &(x, z) in &runtime.blocking_cells {
            grid.set_blocked(x, z, true);
        }
        for pawn in &runtime.pawns {
            grid.set_blocked(pawn.cell_x, pawn.cell_z, true);
        }
        grid.set_blocked(start.0, start.1, false);
        let Some(path) = crate::path::find_path(&grid, start, dest) else {
            return false;
        };
        runtime.pawns[pawn_index].path_cells = path;
        runtime.pawns[pawn_index].path_index = 1;
        true
    }

    fn dynamic_with_overlays(&self) -> Vec<SpriteInput> {
        let mut out = self.base_dynamic_inputs.clone();
        if let Some(runtime) = &self.runtime_state {
            for (sprite, pawn_id) in out.iter_mut().zip(self.base_dynamic_pawn_ids.iter()) {
                let Some(pawn_id) = pawn_id else {
                    continue;
                };
                let Some(pawn) = runtime.pawns.iter().find(|p| p.id == *pawn_id) else {
                    continue;
                };
                let delta = pawn.world_pos - pawn.initial_world_pos;
                sprite.params.world_pos.x += delta.x;
                sprite.params.world_pos.y += delta.y;
            }
            if let Some(selected_id) = runtime.selected_pawn_id
                && let Some(selected) = runtime.pawns.iter().find(|pawn| pawn.id == selected_id)
            {
                for cell in selected.path_cells.iter().skip(selected.path_index) {
                    out.push(SpriteInput {
                        image: self.overlay_image.clone(),
                        params: SpriteParams {
                            world_pos: glam::Vec3::new(
                                cell.0 as f32 + 0.5,
                                cell.1 as f32 + 0.5,
                                -0.23,
                            ),
                            size: Vec2::new(0.36, 0.36),
                            tint: [0.35, 1.00, 0.45, 0.65],
                        },
                    });
                }
            }
        }

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
        if let Some(runtime) = &self.runtime_state {
            if let Some(selected_id) = runtime.selected_pawn_id
                && let Some(selected) = runtime.pawns.iter().find(|pawn| pawn.id == selected_id)
            {
                out.push(SpriteInput {
                    image: self.overlay_image.clone(),
                    params: SpriteParams {
                        world_pos: glam::Vec3::new(
                            selected.world_pos.x,
                            selected.world_pos.y,
                            -0.21,
                        ),
                        size: Vec2::new(1.16, 1.16),
                        tint: [1.00, 0.90, 0.20, 0.30],
                    },
                });
            }
        } else if let Some((x, z)) = self.interaction.selected_cell {
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
        self.base_dynamic_pawn_ids = self.dynamic_sprites.iter().map(|s| s.pawn_id).collect();
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
                    let bounds = self
                        .runtime_state
                        .as_ref()
                        .map(|r| (r.map_width, r.map_height))
                        .or(self.map_bounds);
                    let cell = if let Some((w, h)) = bounds {
                        crate::interaction::world_to_cell_in_bounds(world, w, h)
                    } else {
                        Some(crate::interaction::world_to_cell(world))
                    };
                    if self.interaction.hovered_cell != cell {
                        self.interaction.hovered_cell = cell;
                        if let Some(window) = self.window.as_ref() {
                            window.request_redraw();
                        }
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if self.fixed_step {
                    if let Some(cell) = self.interaction.hovered_cell {
                        let mut selected_hit: Option<usize> = None;
                        let mut move_from_selected: Option<usize> = None;
                        if let Some(runtime) = self.runtime_state.as_ref() {
                            selected_hit = runtime
                                .pawns
                                .iter()
                                .find(|pawn| pawn.cell_x == cell.0 && pawn.cell_z == cell.1)
                                .map(|pawn| pawn.id);
                            if selected_hit.is_none() {
                                move_from_selected = runtime.selected_pawn_id;
                            }
                        }
                        if let Some(hit_pawn_id) = selected_hit {
                            if let Some(runtime) = self.runtime_state.as_mut() {
                                runtime.selected_pawn_id = Some(hit_pawn_id);
                            }
                            self.interaction.selected_cell = Some(cell);
                            info!(
                                "selected pawn id={} at cell=({}, {})",
                                hit_pawn_id, cell.0, cell.1
                            );
                        } else if let Some(selected_pawn_id) = move_from_selected {
                            if self.issue_pawn_move(selected_pawn_id, cell) {
                                info!(
                                    "issued move pawn id={} to cell=({}, {})",
                                    selected_pawn_id, cell.0, cell.1
                                );
                            }
                        } else {
                            self.interaction.selected_cell = Some(cell);
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
                    if let Some(runtime) = self.runtime_state.as_mut() {
                        runtime.selected_pawn_id = None;
                    }
                    self.interaction.selected_cell = None;
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
                    if let Some(runtime) = self.runtime_state.as_mut() {
                        runtime.selected_pawn_id = None;
                    }
                    self.interaction.selected_cell = None;
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
