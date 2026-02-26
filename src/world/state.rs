use crate::fixtures::PawnFacingSpec;

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
}

#[derive(Debug, Clone)]
pub struct WorldState {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<TerrainTile>,
    pub things: Vec<ThingState>,
    pub pawns: Vec<PawnState>,
}
