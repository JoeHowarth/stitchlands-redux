use glam::Vec2;
use std::collections::HashMap;

use crate::cell::Cell;
use crate::path::{PathGrid, find_path};

use super::{PathProgress, WorldState};

pub fn build_path_grid(world: &WorldState) -> PathGrid {
    let mut grid = PathGrid::new(world.width, world.height);
    for thing in &world.things {
        if thing.blocks_movement {
            grid.set_blocked(thing.cell_x, thing.cell_z, true);
        }
    }
    for pawn in &world.pawns {
        grid.set_blocked(pawn.cell_x, pawn.cell_z, true);
    }
    grid
}

pub fn issue_move_intent(world: &mut WorldState, pawn_id: usize, dest: Cell) -> bool {
    let Some(pawn_index) = world.pawns.iter().position(|pawn| pawn.id == pawn_id) else {
        return false;
    };
    let start = Cell::new(
        world.pawns[pawn_index].cell_x,
        world.pawns[pawn_index].cell_z,
    );
    let mut grid = build_path_grid(world);
    grid.set_blocked(start.x, start.z, false);

    let Some(cells) = find_path(&grid, start, dest) else {
        return false;
    };
    world.pawns[pawn_index].path = PathProgress::Following { cells, index: 1 };
    true
}

pub fn tick_world(world: &mut WorldState, dt_seconds: f32) {
    let mut occupied_cells: HashMap<Cell, usize> = world
        .pawns
        .iter()
        .map(|pawn| (Cell::new(pawn.cell_x, pawn.cell_z), pawn.id))
        .collect();

    for pawn in &mut world.pawns {
        let PathProgress::Following { cells, index } = &mut pawn.path else {
            continue;
        };
        if *index >= cells.len() {
            continue;
        }

        let target_cell = cells[*index];
        let blocked_by_thing = world.things.iter().any(|thing| {
            thing.blocks_movement && Cell::new(thing.cell_x, thing.cell_z) == target_cell
        });
        if blocked_by_thing {
            continue;
        }
        if let Some(occupant_id) = occupied_cells.get(&target_cell).copied()
            && occupant_id != pawn.id
        {
            continue;
        }
        let target = Vec2::new(target_cell.x as f32 + 0.5, target_cell.z as f32 + 0.5);
        let to_target = target - pawn.world_pos;
        let distance = to_target.length();
        let max_step = pawn.move_speed_cells_per_sec.max(0.1) * dt_seconds.max(0.0);

        if distance <= max_step {
            let prev_cell = Cell::new(pawn.cell_x, pawn.cell_z);
            pawn.world_pos = target;
            pawn.cell_x = target_cell.x;
            pawn.cell_z = target_cell.z;
            *index += 1;
            occupied_cells.remove(&prev_cell);
            occupied_cells.insert(Cell::new(pawn.cell_x, pawn.cell_z), pawn.id);
        } else if distance > 0.0 {
            pawn.world_pos += to_target / distance * max_step;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cell::Cell;
    use crate::fixtures::{MapSpec, PawnSpawn, SceneFixture, TerrainCell, ThingSpawn};
    use crate::world::{PathProgress, ThingState, WorldState, world_from_fixture};

    use super::{issue_move_intent, tick_world};

    fn fixture_world() -> WorldState {
        world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 8,
                height: 6,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string(),
                    };
                    8 * 6
                ],
            },
            things: vec![ThingSpawn {
                def_name: "ChunkSlagSteel".to_string(),
                cell_x: 3,
                cell_z: 2,
                blocks_movement: true,
            }],
            pawns: vec![PawnSpawn {
                cell_x: 1,
                cell_z: 1,
                label: Some("PawnA".to_string()),
                body: None,
                head: None,
                hair: None,
                beard: None,
                apparel_defs: Vec::new(),
                facing: crate::fixtures::PawnFacingSpec::South,
            }],
            camera: None,
        })
    }

    #[test]
    fn move_intent_creates_non_empty_path() {
        let mut world = fixture_world();
        assert!(issue_move_intent(&mut world, 0, Cell::new(5, 4)));
        let pawn = world
            .pawns
            .iter_mut()
            .find(|pawn| pawn.id == 0)
            .expect("pawn");
        assert!(!pawn.path.is_idle());
    }

    #[test]
    fn tick_moves_pawn_along_path() {
        let mut world = fixture_world();
        assert!(issue_move_intent(&mut world, 0, Cell::new(5, 4)));
        for _ in 0..180 {
            tick_world(&mut world, 1.0 / 60.0);
        }
        let pawn = world
            .pawns
            .iter_mut()
            .find(|pawn| pawn.id == 0)
            .expect("pawn");
        assert_eq!((pawn.cell_x, pawn.cell_z), (5, 4));
    }

    #[test]
    fn blocked_destination_rejected() {
        let mut world = fixture_world();
        assert!(!issue_move_intent(&mut world, 0, Cell::new(3, 2)));
        let pawn = world.pawns.iter().find(|pawn| pawn.id == 0).expect("pawn");
        assert!(pawn.path.is_idle());
    }

    #[test]
    fn zero_length_move_keeps_pawn_stationary() {
        let mut world = fixture_world();
        assert!(issue_move_intent(&mut world, 0, Cell::new(1, 1)));
        tick_world(&mut world, 1.0 / 60.0);
        let pawn = world.pawns.iter().find(|pawn| pawn.id == 0).expect("pawn");
        assert_eq!((pawn.cell_x, pawn.cell_z), (1, 1));
        let PathProgress::Following { cells, index } = &pawn.path else {
            panic!("expected Following");
        };
        assert_eq!(*cells, vec![Cell::new(1, 1)]);
        assert_eq!(*index, 1);
    }

    #[test]
    fn repeated_move_reissue_retargets_path() {
        let mut world = fixture_world();
        assert!(issue_move_intent(&mut world, 0, Cell::new(5, 4)));
        for _ in 0..30 {
            tick_world(&mut world, 1.0 / 60.0);
        }
        assert!(issue_move_intent(&mut world, 0, Cell::new(0, 4)));
        for _ in 0..240 {
            tick_world(&mut world, 1.0 / 60.0);
        }
        let pawn = world.pawns.iter().find(|pawn| pawn.id == 0).expect("pawn");
        assert_eq!((pawn.cell_x, pawn.cell_z), (0, 4));
    }

    #[test]
    fn later_arrival_does_not_enter_occupied_destination_cell() {
        let mut world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 8,
                height: 6,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string(),
                    };
                    8 * 6
                ],
            },
            things: Vec::new(),
            pawns: vec![
                PawnSpawn {
                    cell_x: 0,
                    cell_z: 0,
                    label: Some("PawnA".to_string()),
                    body: None,
                    head: None,
                    hair: None,
                    beard: None,
                    apparel_defs: Vec::new(),
                    facing: crate::fixtures::PawnFacingSpec::South,
                },
                PawnSpawn {
                    cell_x: 3,
                    cell_z: 0,
                    label: Some("PawnB".to_string()),
                    body: None,
                    head: None,
                    hair: None,
                    beard: None,
                    apparel_defs: Vec::new(),
                    facing: crate::fixtures::PawnFacingSpec::South,
                },
            ],
            camera: None,
        });

        assert!(issue_move_intent(&mut world, 0, Cell::new(4, 0)));
        assert!(issue_move_intent(&mut world, 1, Cell::new(4, 0)));
        for _ in 0..300 {
            tick_world(&mut world, 1.0 / 60.0);
        }

        let pawn_a = world
            .pawns
            .iter()
            .find(|pawn| pawn.id == 0)
            .expect("pawn a");
        let pawn_b = world
            .pawns
            .iter()
            .find(|pawn| pawn.id == 1)
            .expect("pawn b");
        assert_eq!((pawn_b.cell_x, pawn_b.cell_z), (4, 0));
        assert_ne!((pawn_a.cell_x, pawn_a.cell_z), (4, 0));
        assert_ne!(
            (pawn_a.cell_x, pawn_a.cell_z),
            (pawn_b.cell_x, pawn_b.cell_z)
        );
    }

    #[test]
    fn pawn_does_not_step_into_newly_blocked_thing_cell() {
        let mut world = fixture_world();
        assert!(issue_move_intent(&mut world, 0, Cell::new(5, 4)));
        world.things.push(ThingState {
            id: 999,
            def_name: "LateBlocker".to_string(),
            cell_x: 5,
            cell_z: 4,
            blocks_movement: true,
        });

        for _ in 0..300 {
            tick_world(&mut world, 1.0 / 60.0);
        }

        let pawn = world.pawns.iter().find(|pawn| pawn.id == 0).expect("pawn");
        assert_ne!((pawn.cell_x, pawn.cell_z), (5, 4));
    }
}
