use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SceneFixture {
    pub schema_version: u32,
    pub map: MapSpec,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerrainCell {
    pub terrain_def: String,
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
