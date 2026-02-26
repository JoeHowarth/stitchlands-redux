use glam::Vec2;

pub fn world_to_cell(world: Vec2) -> (i32, i32) {
    (world.x.floor() as i32, world.y.floor() as i32)
}

#[cfg(test)]
mod tests {
    use glam::Vec2;

    use super::world_to_cell;

    #[test]
    fn floors_world_to_cell() {
        assert_eq!(world_to_cell(Vec2::new(3.8, 2.1)), (3, 2));
        assert_eq!(world_to_cell(Vec2::new(0.0, 0.0)), (0, 0));
    }

    #[test]
    fn handles_negative_world_coords() {
        assert_eq!(world_to_cell(Vec2::new(-0.1, 2.0)), (-1, 2));
    }
}
