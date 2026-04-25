use crate::cell::Cell;
use crate::defs::RgbaColor;
use crate::pawn::PawnFacing;
use glam::Vec2;

#[derive(Debug, Clone)]
pub struct TerrainTile {
    pub terrain_def: String,
}

#[derive(Debug, Clone)]
pub struct ThingState {
    pub id: usize,
    pub def_name: String,
    pub cell_x: i32,
    pub cell_z: i32,
    pub blocks_movement: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RoofTile {
    pub roofed: bool,
    pub thick: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct GlowSource {
    pub cell_x: i32,
    pub cell_z: i32,
    pub radius: f32,
    pub color: RgbaColor,
    pub overlight_radius: f32,
}

#[derive(Debug, Clone)]
pub struct RenderState {
    pub roofs: Vec<RoofTile>,
    pub fog: Vec<bool>,
    pub snow_depth: Vec<f32>,
    pub day_percent: Option<f32>,
    pub sky_glow: Option<RgbaColor>,
    pub shadow_color: Option<RgbaColor>,
    pub shadow_vector: Option<Vec2>,
    pub glow_sources: Vec<GlowSource>,
}

#[derive(Debug, Clone, Default)]
pub enum PathProgress {
    #[default]
    Idle,
    Following {
        cells: Vec<Cell>,
        index: usize,
    },
}

impl PathProgress {
    pub fn is_idle(&self) -> bool {
        match self {
            Self::Idle => true,
            Self::Following { cells, index } => *index >= cells.len(),
        }
    }

    pub fn remaining_cells(&self) -> &[Cell] {
        match self {
            Self::Idle => &[],
            Self::Following { cells, index } => {
                if *index < cells.len() {
                    &cells[*index..]
                } else {
                    &[]
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PawnState {
    pub id: usize,
    pub label: String,
    pub cell_x: i32,
    pub cell_z: i32,
    pub facing: PawnFacing,
    pub body: Option<String>,
    pub head: Option<String>,
    pub hair: Option<String>,
    pub beard: Option<String>,
    pub apparel_defs: Vec<String>,
    pub world_pos: Vec2,
    pub path: PathProgress,
    pub move_speed_cells_per_sec: f32,
}

#[derive(Debug, Clone)]
pub struct WorldState {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) terrain: Vec<TerrainTile>,
    pub(super) render_state: RenderState,
    pub(super) things: Vec<ThingState>,
    pub(super) pawns: Vec<PawnState>,
    /// Index `z * width + x` -> indices into `things`. Stays in sync with
    /// `things` because v2 doesn't move things at runtime; rebuild if that
    /// ever changes.
    pub(super) thing_grid: Vec<Vec<usize>>,
}

impl WorldState {
    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn terrain(&self) -> &[TerrainTile] {
        &self.terrain
    }
    pub fn render_state(&self) -> &RenderState {
        &self.render_state
    }
    pub fn things(&self) -> &[ThingState] {
        &self.things
    }
    pub fn pawns(&self) -> &[PawnState] {
        &self.pawns
    }
}
