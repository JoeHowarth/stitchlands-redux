use crate::cell::Cell;
use crate::fixtures::PawnFacingSpec;
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

#[derive(Debug, Clone)]
pub struct PawnState {
    pub id: usize,
    pub label: String,
    pub cell_x: i32,
    pub cell_z: i32,
    pub facing: PawnFacingSpec,
    pub body: Option<String>,
    pub head: Option<String>,
    pub hair: Option<String>,
    pub beard: Option<String>,
    pub apparel_defs: Vec<String>,
    pub world_pos: Vec2,
    pub path_cells: Vec<Cell>,
    pub path_index: usize,
    pub move_speed_cells_per_sec: f32,
}

#[derive(Debug, Clone)]
pub struct WorldState {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) terrain: Vec<TerrainTile>,
    pub(super) things: Vec<ThingState>,
    pub(super) pawns: Vec<PawnState>,
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
    pub fn things(&self) -> &[ThingState] {
        &self.things
    }
    pub fn pawns(&self) -> &[PawnState] {
        &self.pawns
    }
}
