mod loader;
mod schema;
mod validate;

pub use loader::load_fixture;
pub use schema::{PawnFacingSpec, SceneFixture};
pub use validate::validate_fixture;
