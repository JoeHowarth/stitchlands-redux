use glam::Vec2;

pub fn world_to_cell(world: Vec2) -> (i32, i32) {
    (world.x.floor() as i32, world.y.floor() as i32)
}

pub fn world_to_cell_in_bounds(world: Vec2, width: usize, height: usize) -> Option<(i32, i32)> {
    let cell = world_to_cell(world);
    if cell.0 < 0 || cell.1 < 0 || cell.0 >= width as i32 || cell.1 >= height as i32 {
        return None;
    }
    Some(cell)
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::{world_to_cell, world_to_cell_in_bounds};

    #[test]
    fn floors_world_to_cell() {
        assert_eq!(world_to_cell(Vec2::new(3.8, 2.1)), (3, 2));
        assert_eq!(world_to_cell(Vec2::new(0.0, 0.0)), (0, 0));
    }

    #[test]
    fn handles_negative_world_coords() {
        assert_eq!(world_to_cell(Vec2::new(-0.1, 2.0)), (-1, 2));
    }

    #[test]
    fn bounds_check_works() {
        assert_eq!(
            world_to_cell_in_bounds(Vec2::new(3.2, 4.7), 8, 8),
            Some((3, 4))
        );
        assert_eq!(world_to_cell_in_bounds(Vec2::new(8.0, 4.7), 8, 8), None);
    }
}
