#[derive(Debug, Clone)]
pub struct PathGrid {
    width: usize,
    height: usize,
    blocked: Vec<bool>,
}

impl PathGrid {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            blocked: vec![false; width.saturating_mul(height)],
        }
    }

    pub fn in_bounds(&self, x: i32, z: i32) -> bool {
        x >= 0 && z >= 0 && x < self.width as i32 && z < self.height as i32
    }

    pub fn is_blocked(&self, x: i32, z: i32) -> bool {
        if !self.in_bounds(x, z) {
            return true;
        }
        self.blocked[z as usize * self.width + x as usize]
    }

    pub fn set_blocked(&mut self, x: i32, z: i32, blocked: bool) {
        if !self.in_bounds(x, z) {
            return;
        }
        self.blocked[z as usize * self.width + x as usize] = blocked;
    }
}
