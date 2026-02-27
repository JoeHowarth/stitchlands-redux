mod loader;
mod schema;
mod validate;

pub use loader::load_fixture;
#[cfg(test)]
pub use schema::{MapSpec, PawnSpawn, TerrainCell, ThingSpawn};
pub use schema::SceneFixture;
pub use validate::validate_fixture;
