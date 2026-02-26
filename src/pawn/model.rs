use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PawnFacing {
    North,
    East,
    South,
    West,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ApparelLayer {
    OnSkin,
    Middle,
    Shell,
    Belt,
    Overhead,
    EyeCover,
}

impl ApparelLayer {
    pub const ALL: [Self; 6] = [
        Self::OnSkin,
        Self::Middle,
        Self::Shell,
        Self::Belt,
        Self::Overhead,
        Self::EyeCover,
    ];

    pub fn draw_order(self) -> i32 {
        match self {
            Self::OnSkin => 10,
            Self::Middle => 20,
            Self::Shell => 30,
            Self::Belt => 40,
            Self::Overhead => 50,
            Self::EyeCover => 60,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApparelRenderInput {
    pub label: String,
    pub tex_path: String,
    pub layer: ApparelLayer,
    pub covers_upper_head: bool,
    pub covers_full_head: bool,
    pub draw_size: Vec2,
    pub tint: [f32; 4],
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct PawnDrawFlags {
    pub hide_hair: bool,
    pub hide_beard: bool,
    pub hide_head: bool,
    pub head_stump: bool,
}

impl PawnDrawFlags {
    pub const NONE: Self = Self {
        hide_hair: false,
        hide_beard: false,
        hide_head: false,
        head_stump: false,
    };
}

#[derive(Debug, Clone)]
pub struct PawnRenderInput {
    pub label: String,
    pub facing: PawnFacing,
    pub world_pos: Vec3,
    pub body_tex_path: String,
    pub head_tex_path: Option<String>,
    pub hair_tex_path: Option<String>,
    pub beard_tex_path: Option<String>,
    pub body_size: Vec2,
    pub head_size: Vec2,
    pub hair_size: Vec2,
    pub beard_size: Vec2,
    pub tint: [f32; 4],
    pub apparel: Vec<ApparelRenderInput>,
    pub draw_flags: PawnDrawFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct LayeringProfile {
    pub body_z: f32,
    pub head_z: f32,
    pub hair_z: f32,
    pub beard_z: f32,
    pub apparel_body_base_z: f32,
    pub apparel_head_base_z: f32,
    pub apparel_step_z: f32,
}

impl Default for LayeringProfile {
    fn default() -> Self {
        Self {
            body_z: -0.60,
            head_z: -0.58,
            hair_z: -0.565,
            beard_z: -0.562,
            apparel_body_base_z: -0.59,
            apparel_head_base_z: -0.553,
            apparel_step_z: 0.0008,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PawnComposeConfig {
    pub layering: LayeringProfile,
}
