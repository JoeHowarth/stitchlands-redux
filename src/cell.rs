/// A discrete grid cell coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    pub x: i32,
    pub z: i32,
}

impl Cell {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}
