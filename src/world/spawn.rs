use crate::fixtures::SceneFixture;
use glam::Vec2;

use super::{PathProgress, PawnState, TerrainTile, ThingState, WorldState};

pub fn world_from_fixture(fixture: &SceneFixture) -> WorldState {
    let terrain = fixture
        .map
        .terrain
        .iter()
        .map(|tile| TerrainTile {
            terrain_def: tile.terrain_def.clone(),
        })
        .collect();

    let things: Vec<ThingState> = fixture
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

    let cell_count = fixture.map.width * fixture.map.height;
    let mut thing_grid: Vec<Vec<usize>> = vec![Vec::new(); cell_count];
    for (thing_idx, thing) in things.iter().enumerate() {
        if thing.cell_x < 0 || thing.cell_z < 0 {
            continue;
        }
        let (x, z) = (thing.cell_x as usize, thing.cell_z as usize);
        if x >= fixture.map.width || z >= fixture.map.height {
            continue;
        }
        thing_grid[z * fixture.map.width + x].push(thing_idx);
    }

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
            path: PathProgress::Idle,
            move_speed_cells_per_sec: 3.0,
        })
        .collect();

    WorldState {
        width: fixture.map.width,
        height: fixture.map.height,
        terrain,
        things,
        pawns,
        thing_grid,
    }
}
