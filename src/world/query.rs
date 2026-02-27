use crate::cell::Cell;

use super::{PawnState, WorldState};

pub fn pawn_id_at_cell(world: &WorldState, cell: Cell) -> Option<usize> {
    world
        .pawns
        .iter()
        .find(|pawn| pawn.cell_x == cell.x && pawn.cell_z == cell.z)
        .map(|pawn| pawn.id)
}

pub fn pawn_by_id(world: &WorldState, pawn_id: usize) -> Option<&PawnState> {
    world.pawns.iter().find(|pawn| pawn.id == pawn_id)
}

pub fn selected_pawn(world: &WorldState, pawn_id: Option<usize>) -> Option<&PawnState> {
    pawn_id.and_then(|id| pawn_by_id(world, id))
}

pub fn pawn_is_idle(world: &WorldState, pawn_id: usize) -> Option<bool> {
    pawn_by_id(world, pawn_id).map(|pawn| pawn.path_index >= pawn.path_cells.len())
}
