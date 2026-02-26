use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum PawnNodeKind {
    Body,
    Head,
    Stump,
    Hair,
    Beard,
    Apparel,
}

#[derive(Debug, Clone)]
pub struct PawnNode {
    pub id: String,
    pub kind: PawnNodeKind,
    pub tex_path: String,
    pub world_pos: Vec3,
    pub size: Vec2,
    pub tint: [f32; 4],
    pub z: f32,
    pub order: usize,
}
