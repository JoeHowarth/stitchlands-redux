mod loader;
mod schema;
mod validate;

pub use loader::load_fixture;
pub use schema::{CameraSpec, MapSpec, PawnSpawn, TerrainCell, ThingSpawn};
pub use schema::SceneFixture;
pub use validate::validate_fixture;
