use glam::Vec2;

use crate::cell::Cell;

pub fn world_to_cell(world: Vec2) -> Cell {
    Cell::new(world.x.floor() as i32, world.y.floor() as i32)
}

pub fn world_to_cell_in_bounds(world: Vec2, width: usize, height: usize) -> Option<Cell> {
    let cell = world_to_cell(world);
    if cell.x < 0 || cell.z < 0 || cell.x >= width as i32 || cell.z >= height as i32 {
        return None;
    }
    Some(cell)
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use crate::cell::Cell;

    use super::{world_to_cell, world_to_cell_in_bounds};

    #[test]
    fn floors_world_to_cell() {
        assert_eq!(world_to_cell(Vec2::new(3.8, 2.1)), Cell::new(3, 2));
        assert_eq!(world_to_cell(Vec2::new(0.0, 0.0)), Cell::new(0, 0));
    }

    #[test]
    fn handles_negative_world_coords() {
        assert_eq!(world_to_cell(Vec2::new(-0.1, 2.0)), Cell::new(-1, 2));
    }

    #[test]
    fn bounds_check_works() {
        assert_eq!(
            world_to_cell_in_bounds(Vec2::new(3.2, 4.7), 8, 8),
            Some(Cell::new(3, 4))
        );
        assert_eq!(world_to_cell_in_bounds(Vec2::new(8.0, 4.7), 8, 8), None);
    }
}
