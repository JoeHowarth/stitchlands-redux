use glam::{Vec2, Vec3};

#[derive(Debug, Clone)]
pub struct PawnNode {
    pub id: String,
    pub tex_path: String,
    pub world_pos: Vec3,
    pub size: Vec2,
    pub tint: [f32; 4],
    pub z: f32,
    pub order: usize,
}
