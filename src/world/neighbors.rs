use crate::cell::Cell;

#[cfg(test)]
use super::ThingState;
use super::WorldState;

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
