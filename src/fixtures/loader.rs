use std::path::Path;

use anyhow::{Context, Result};
use ron::extensions::Extensions;

use super::{SceneFixture, validate_fixture};

pub fn load_fixture(path: &Path) -> Result<SceneFixture> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading fixture {}", path.display()))?;
    let fixture: SceneFixture = ron::Options::default()
        .with_default_extension(Extensions::IMPLICIT_SOME)
        .from_str(&raw)
        .with_context(|| format!("parsing RON fixture {}", path.display()))?;
    validate_fixture(&fixture)?;
    Ok(fixture)
}

#[cfg(test)]
mod tests {
    use super::load_fixture;

    #[test]
    fn parses_and_validates_fixture() {
        let path = std::path::Path::new("fixtures/v2/move_lane.ron");
        let fixture = load_fixture(path).expect("fixture should parse");
        assert_eq!(fixture.schema_version, 2);
        assert_eq!(
            fixture.map.width * fixture.map.height,
            fixture.map.terrain.len()
        );
    }

    #[test]
    fn parses_walls_patterns_fixture() {
        let path = std::path::Path::new("fixtures/v2/walls_patterns.ron");
        let fixture = load_fixture(path).expect("walls_patterns.ron should parse");
        assert_eq!(
            fixture.map.width * fixture.map.height,
            fixture.map.terrain.len(),
            "terrain count must match width*height"
        );
        assert!(
            fixture.things.iter().all(|t| t.def_name == "Wall"),
            "walls_patterns is wall-only"
        );
    }
}
