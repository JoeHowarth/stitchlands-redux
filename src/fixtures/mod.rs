mod loader;
mod schema;
mod validate;

pub use loader::load_fixture;
pub use schema::SceneFixture;
#[cfg(test)]
pub use schema::{
    FixtureColor, FixtureVector2, GlowSourceSpec, MapSpec, PawnSpawn, RenderSpec, RoofCell,
    TerrainCell, ThingSpawn,
};
pub use validate::validate_fixture;
