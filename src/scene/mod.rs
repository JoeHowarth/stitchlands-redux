use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use image::RgbaImage;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Layer {
    BeforeWorld,
    Terrain,
    AfterTerrain,
    StaticThings,
    AfterStatic,
    Dynamic,
    AfterDynamic,
}

pub const LAYER_SEQUENCE: [Layer; 7] = [
    Layer::BeforeWorld,
    Layer::Terrain,
    Layer::AfterTerrain,
    Layer::StaticThings,
    Layer::AfterStatic,
    Layer::Dynamic,
    Layer::AfterDynamic,
];

impl Layer {
    pub fn ordering(self) -> LayerOrdering {
        match self {
            Self::Terrain => LayerOrdering::ByTerrainPrecedence,
            Self::StaticThings | Self::Dynamic => LayerOrdering::ByAltitudeThenZ,
            Self::BeforeWorld | Self::AfterTerrain | Self::AfterStatic | Self::AfterDynamic => {
                LayerOrdering::InsertionOrder
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LayerOrdering {
    ByTerrainPrecedence,
    ByAltitudeThenZ,
    InsertionOrder,
    Explicit,
}

pub const LAYER_ORDERINGS: [LayerOrdering; 4] = [
    LayerOrdering::ByTerrainPrecedence,
    LayerOrdering::ByAltitudeThenZ,
    LayerOrdering::InsertionOrder,
    LayerOrdering::Explicit,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MaterialKind {
    Cutout,
    Terrain,
    TerrainEdge,
    TerrainWater,
    WaterDepth,
    LightOverlay,
    EdgeShadow,
    SunShadow,
    FogOfWar,
    Snow,
    Transparent,
    SolidColor,
}

pub const MATERIAL_KIND_INITIAL_SET: [MaterialKind; 12] = [
    MaterialKind::Cutout,
    MaterialKind::Terrain,
    MaterialKind::TerrainEdge,
    MaterialKind::TerrainWater,
    MaterialKind::WaterDepth,
    MaterialKind::LightOverlay,
    MaterialKind::EdgeShadow,
    MaterialKind::SunShadow,
    MaterialKind::FogOfWar,
    MaterialKind::Snow,
    MaterialKind::Transparent,
    MaterialKind::SolidColor,
];

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OverlayBlendMode {
    Alpha,
    Multiply,
    SunShadow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunShadowParams {
    pub shadow_vector: [f32; 2],
    pub shadow_strength: f32,
    pub material_color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct ColoredMeshInput {
    pub layer: Layer,
    pub material: MaterialKind,
    pub blend_mode: OverlayBlendMode,
    pub sun_shadow: Option<SunShadowParams>,
    pub vertices: Vec<ColoredVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct TexturedMeshInput {
    pub layer: Layer,
    pub material: MaterialKind,
    pub image: RgbaImage,
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u32>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ColoredVertex {
    pub world_pos: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct TexturedVertex {
    pub world_pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct SpriteInput {
    pub image: RgbaImage,
    pub params: SpriteParams,
    pub material: MaterialKind,
}

#[derive(Debug, Clone)]
pub struct SpriteRecord {
    pub texture: TextureHandle,
    pub params: SpriteParams,
    pub material: MaterialKind,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TextureHandle(pub(crate) u32);

/// UV sub-rect `(u_min, v_min, u_max, v_max)` covering the full texture.
/// For atlas-indexed sprites, use `linking::atlas_uv_rect` or similar helpers.
pub const FULL_UV_RECT: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Edge-overlay fan submitted to the edge pipeline. The image is the
/// neighbor terrain's base texture; the fan's per-vertex `alpha` drives a
/// radial fade from the matching perimeter verts toward the center.
#[derive(Debug, Clone)]
pub struct EdgeSpriteInput {
    pub image: RgbaImage,
    pub fan: EdgeFan,
    pub material: MaterialKind,
}

#[derive(Debug, Clone)]
pub struct EdgeFanInstance {
    pub texture: TextureHandle,
    pub fan: EdgeFan,
    pub material: MaterialKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    FadeRough = 1,
    Water = 2,
}

/// Triangle indices (per fan) for the 8 fan triangles (m, (m+1)%8, 8).
pub const FAN_TRI_INDICES: [u32; 24] = [
    0, 1, 8, 1, 2, 8, 2, 3, 8, 3, 4, 8, 4, 5, 8, 5, 6, 8, 6, 7, 8, 7, 0, 8,
];

/// 9-vertex fan for a single overlay contribution. Vertex order is
/// (0 S mid, 1 SW, 2 W mid, 3 NW, 4 N mid, 5 NE, 6 E mid, 7 SE, 8 center).
/// Center alpha is always 0.
#[derive(Debug, Clone)]
pub struct EdgeFan {
    pub vertices: [EdgeVertex; 9],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct EdgeVertex {
    pub world_pos: [f32; 3],
    pub uv: [f32; 2],
    pub alpha: f32,
    pub noise_seed: [f32; 2],
    pub tint: [f32; 4],
    pub edge_type: u32,
    pub _pad: u32,
}

#[derive(Debug, Clone)]
pub struct SpriteParams {
    pub world_pos: Vec3,
    pub size: Vec2,
    pub tint: [f32; 4],
    /// Sub-rect of the texture to sample, as `(u_min, v_min, u_max, v_max)`.
    /// Use `FULL_UV_RECT` for whole-texture sampling.
    pub uv_rect: [f32; 4],
}
