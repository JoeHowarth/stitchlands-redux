use crate::fixtures::SceneFixture;
use glam::Vec2;

use super::{PawnState, TerrainTile, ThingState, WorldState};

pub fn world_from_fixture(fixture: &SceneFixture) -> WorldState {
    let terrain = fixture
        .map
        .terrain
        .iter()
        .map(|tile| TerrainTile {
            terrain_def: tile.terrain_def.clone(),
        })
        .collect();

    let things = fixture
        .things
        .iter()
        .enumerate()
        .map(|(id, thing)| ThingState {
            id,
            def_name: thing.def_name.clone(),
            cell_x: thing.cell_x,
            cell_z: thing.cell_z,
            blocks_movement: thing.blocks_movement,
        })
        .collect();

    let pawns = fixture
        .pawns
        .iter()
        .enumerate()
        .map(|(id, pawn)| PawnState {
            id,
            label: pawn
                .label
                .clone()
                .unwrap_or_else(|| format!("Pawn{}", id + 1)),
            cell_x: pawn.cell_x,
            cell_z: pawn.cell_z,
            facing: pawn.facing,
            body: pawn.body.clone(),
            head: pawn.head.clone(),
            hair: pawn.hair.clone(),
            beard: pawn.beard.clone(),
            apparel_defs: pawn.apparel_defs.clone(),
            world_pos: Vec2::new(pawn.cell_x as f32 + 0.5, pawn.cell_z as f32 + 0.5),
            path_cells: Vec::new(),
            path_index: 0,
            move_speed_cells_per_sec: 3.0,
        })
        .collect();

    WorldState {
        width: fixture.map.width,
        height: fixture.map.height,
        terrain,
        things,
        pawns,
    }
}
