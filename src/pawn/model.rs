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
    pub explicit_skip_hair: bool,
    pub explicit_skip_beard: bool,
    pub has_explicit_skip_flags: bool,
    pub covers_upper_head: bool,
    pub covers_full_head: bool,
    pub draw_offset: Vec2,
    pub draw_scale: Vec2,
    pub layer_override: Option<f32>,
    pub draw_size: Vec2,
    pub tint: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct BodyTypeRenderData {
    pub head_offset: Vec2,
    pub body_size_factor: f32,
}

impl Default for BodyTypeRenderData {
    fn default() -> Self {
        Self {
            head_offset: Vec2::new(0.0, 0.22),
            body_size_factor: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeadTypeRenderData {
    pub narrow: bool,
    pub narrow_crown_horizontal_offset: f32,
    pub beard_offset: Vec3,
    pub beard_offset_x_east: f32,
}

impl Default for HeadTypeRenderData {
    fn default() -> Self {
        Self {
            narrow: false,
            narrow_crown_horizontal_offset: 0.0,
            beard_offset: Vec3::ZERO,
            beard_offset_x_east: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BeardTypeRenderData {
    pub offset_narrow_east: Vec3,
    pub offset_narrow_south: Vec3,
}

impl Default for BeardTypeRenderData {
    fn default() -> Self {
        Self {
            offset_narrow_east: Vec3::ZERO,
            offset_narrow_south: Vec3::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OverlayAnchor {
    Body,
    Head,
}

#[derive(Debug, Clone)]
pub struct HediffOverlayInput {
    pub label: String,
    pub tex_path: String,
    pub anchor: OverlayAnchor,
    pub layer_offset: i32,
    pub draw_size: Vec2,
    pub tint: [f32; 4],
    pub required_body_part_group: Option<String>,
    pub visible_facing: Option<Vec<PawnFacing>>,
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
    pub stump_tex_path: Option<String>,
    pub hair_tex_path: Option<String>,
    pub beard_tex_path: Option<String>,
    pub body_size: Vec2,
    pub head_size: Vec2,
    pub stump_size: Vec2,
    pub hair_size: Vec2,
    pub beard_size: Vec2,
    pub body_type: BodyTypeRenderData,
    pub head_type: HeadTypeRenderData,
    pub beard_type: BeardTypeRenderData,
    pub tint: [f32; 4],
    pub apparel: Vec<ApparelRenderInput>,
    pub present_body_part_groups: Vec<String>,
    pub hediff_overlays: Vec<HediffOverlayInput>,
    pub draw_flags: PawnDrawFlags,
}

#[derive(Debug, Clone, Copy)]
pub struct LayeringProfile {
    pub body_z: f32,
    pub head_z: f32,
    pub hair_z: f32,
    pub beard_z: f32,
    pub hair_y_offset: f32,
    pub stump_y_offset: f32,
    pub apparel_body_base_z: f32,
    pub apparel_head_base_z: f32,
    pub apparel_head_y_offset: f32,
    pub apparel_step_z: f32,
    pub hediff_body_base_z: f32,
    pub hediff_head_base_z: f32,
    pub hediff_head_y_offset: f32,
    pub hediff_step_z: f32,
}

impl Default for LayeringProfile {
    fn default() -> Self {
        Self {
            body_z: -0.60,
            head_z: -0.581_707_3,
            hair_z: -0.577_317_06,
            beard_z: -0.578_048_77,
            hair_y_offset: 0.0,
            stump_y_offset: 0.0,
            apparel_body_base_z: -0.592_682_9,
            apparel_head_base_z: -0.574_390_3,
            apparel_head_y_offset: 0.0,
            apparel_step_z: 0.000_365_853_7,
            hediff_body_base_z: -0.597_073_2,
            hediff_head_base_z: -0.576_219_5,
            hediff_head_y_offset: 0.0,
            hediff_step_z: 0.000_365_853_7,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PawnComposeConfig {
    pub layering: LayeringProfile,
}
