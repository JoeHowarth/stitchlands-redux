use crate::cell::Cell;

#[cfg(test)]
use super::ThingState;
use super::WorldState;

pub const DEPTH_WALL: f32 = -0.70;
pub const DEPTH_WALL_CORNER: f32 = -0.69;

/// Order: N, E, S, W. Matches RimWorld's `GenAdj.CardinalDirections`; the
/// `link_index` bitmask in `crate::linking` is keyed off this same ordering.
const CARDINAL_OFFSETS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// Order: NE, SE, SW, NW.
const DIAGONAL_OFFSETS: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, -1), (-1, 1)];

pub fn cardinal_neighbors(cell: Cell) -> [Cell; 4] {
    CARDINAL_OFFSETS.map(|(dx, dz)| Cell::new(cell.x + dx, cell.z + dz))
}

pub fn diagonal_neighbors(cell: Cell) -> [Cell; 4] {
    DIAGONAL_OFFSETS.map(|(dx, dz)| Cell::new(cell.x + dx, cell.z + dz))
}

impl WorldState {
    pub fn cell_in_bounds(&self, cell: Cell) -> bool {
        cell.x >= 0
            && cell.z >= 0
            && (cell.x as usize) < self.width
            && (cell.z as usize) < self.height
    }

    pub fn cell_index(&self, cell: Cell) -> Option<usize> {
        if !self.cell_in_bounds(cell) {
            return None;
        }
        Some(cell.z as usize * self.width + cell.x as usize)
    }

    pub fn things_at(&self, cell: Cell) -> &[usize] {
        match self.cell_index(cell) {
            Some(idx) => &self.thing_grid[idx],
            None => &[],
        }
    }

    #[cfg(test)]
    pub(crate) fn push_thing(&mut self, thing: ThingState) {
        let cell = Cell::new(thing.cell_x, thing.cell_z);
        let thing_idx = self.things.len();
        self.things.push(thing);
        if let Some(grid_idx) = self.cell_index(cell) {
            self.thing_grid[grid_idx].push(thing_idx);
        }
    }
}
