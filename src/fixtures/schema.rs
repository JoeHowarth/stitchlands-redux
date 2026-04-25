use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SceneFixture {
    pub schema_version: u32,
    pub map: MapSpec,
    #[serde(default)]
    pub render: RenderSpec,
    #[serde(default)]
    pub things: Vec<ThingSpawn>,
    #[serde(default)]
    pub pawns: Vec<PawnSpawn>,
    pub camera: Option<CameraSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MapSpec {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<TerrainCell>,
    #[serde(default)]
    pub roofs: Vec<RoofCell>,
    #[serde(default)]
    pub fog: Vec<bool>,
    #[serde(default)]
    pub snow_depth: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerrainCell {
    pub terrain_def: String,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct RoofCell {
    #[serde(default)]
    pub roofed: bool,
    #[serde(default)]
    pub thick: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RenderSpec {
    #[serde(default)]
    pub day_percent: Option<f32>,
    #[serde(default)]
    pub sky_glow: Option<FixtureColor>,
    #[serde(default)]
    pub shadow_color: Option<FixtureColor>,
    #[serde(default)]
    pub shadow_vector: Option<FixtureVector2>,
    #[serde(default)]
    pub glow_sources: Vec<GlowSourceSpec>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct FixtureColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    #[serde(default = "default_color_alpha")]
    pub a: f32,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct FixtureVector2 {
    pub x: f32,
    pub z: f32,
}

impl From<FixtureColor> for crate::defs::RgbaColor {
    fn from(value: FixtureColor) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GlowSourceSpec {
    pub cell_x: i32,
    pub cell_z: i32,
    pub radius: f32,
    pub color: FixtureColor,
    #[serde(default)]
    pub overlight_radius: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThingSpawn {
    pub def_name: String,
    pub cell_x: i32,
    pub cell_z: i32,
    #[serde(default = "default_true")]
    pub blocks_movement: bool,
}

/// Pawn definition in a fixture scene.
///
/// `body`, `head`, `hair`, and `beard` are XML **defNames**, not texture path
/// segments. These are often different from what appears in the graphicPath:
///
///   defName: "Male_AverageNormal"   graphicPath: ".../Male_Average_Normal"
///   defName: "Full"                 graphicPath: ".../Beard_Full"
///   defName: "Shaved"               graphicPath: ".../Shaved"
///
/// Check `Core/Defs/` XML files for the correct defName when authoring fixtures.
#[derive(Debug, Clone, Deserialize)]
pub struct PawnSpawn {
    pub cell_x: i32,
    pub cell_z: i32,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub hair: Option<String>,
    #[serde(default)]
    pub beard: Option<String>,
    #[serde(default)]
    pub apparel_defs: Vec<String>,
    #[serde(default)]
    pub facing: crate::pawn::PawnFacing,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CameraSpec {
    pub center_x: f32,
    pub center_z: f32,
}

const fn default_true() -> bool {
    true
}

const fn default_color_alpha() -> f32 {
    1.0
}
