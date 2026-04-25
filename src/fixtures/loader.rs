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
    fn parses_terrain_mix_fixture() {
        let path = std::path::Path::new("fixtures/v2/terrain_mix.ron");
        let fixture = load_fixture(path).expect("terrain_mix.ron should parse");
        assert_eq!(
            fixture.map.width * fixture.map.height,
            fixture.map.terrain.len(),
            "terrain count must match width*height"
        );
        assert!(
            fixture
                .map
                .terrain
                .iter()
                .any(|c| c.terrain_def == "WaterShallow"),
            "terrain_mix must include WaterShallow pocket"
        );
        assert!(
            fixture
                .map
                .terrain
                .iter()
                .any(|c| c.terrain_def == "WaterDeep"),
            "terrain_mix must include WaterDeep center"
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

    #[test]
    fn parses_lighting_overlay_fixture() {
        let path = std::path::Path::new("fixtures/v2/lighting_overlay.ron");
        let fixture = load_fixture(path).expect("lighting_overlay.ron should parse");
        assert_eq!(
            fixture.map.width * fixture.map.height,
            fixture.map.roofs.len()
        );
        assert_eq!(
            fixture.map.width * fixture.map.height,
            fixture.map.snow_depth.len()
        );
        assert_eq!(fixture.render.glow_sources.len(), 1);
    }

    #[test]
    fn parses_shadow_data_fixture() {
        let path = std::path::Path::new("fixtures/v2/shadow_data.ron");
        let fixture = load_fixture(path).expect("shadow_data.ron should parse");
        assert_eq!(fixture.things.len(), 2);
        assert!(
            fixture
                .things
                .iter()
                .any(|thing| thing.def_name == "WoodFiredGenerator" && thing.blocks_movement)
        );
        assert!(
            fixture
                .things
                .iter()
                .any(|thing| thing.def_name == "Plant_Bush" && !thing.blocks_movement)
        );
    }

    #[test]
    fn parses_glower_lighting_fixture() {
        let path = std::path::Path::new("fixtures/v2/glower_lighting.ron");
        let fixture = load_fixture(path).expect("glower_lighting.ron should parse");
        assert_eq!(fixture.render.day_percent, Some(0.0));
        assert!(fixture.render.glow_sources.is_empty());
        assert_eq!(fixture.things[0].def_name, "Gloomlight");
    }

    #[test]
    fn parses_render_state_fields() {
        let path = std::env::temp_dir().join(format!(
            "stitchlands-render-state-fixture-{}.ron",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"
(
  schema_version: 2,
  map: (
    width: 2,
    height: 2,
    terrain: [
      (terrain_def: "Soil"), (terrain_def: "Soil"),
      (terrain_def: "Soil"), (terrain_def: "Soil"),
    ],
    roofs: [
      (roofed: false), (roofed: true),
      (roofed: true, thick: true), (roofed: false),
    ],
    fog: [false, true, true, false],
    snow_depth: [0.0, 0.25, 0.5, 1.0],
  ),
  render: (
    day_percent: 0.5,
    sky_glow: (r: 0.8, g: 0.7, b: 0.6, a: 1.0),
    shadow_color: (r: 0.1, g: 0.1, b: 0.2, a: 0.5),
    shadow_vector: (x: 0.35, z: -0.4),
    glow_sources: [
      (
        cell_x: 1,
        cell_z: 0,
        radius: 6.0,
        color: (r: 255.0, g: 240.0, b: 180.0, a: 0.0),
        overlight_radius: 2.5,
      ),
    ],
  ),
)
"#,
        )
        .unwrap();

        let fixture = load_fixture(&path).expect("fixture should parse");
        let _ = std::fs::remove_file(path);

        assert!(fixture.map.roofs[1].roofed);
        assert!(fixture.map.roofs[2].thick);
        assert_eq!(fixture.map.fog, vec![false, true, true, false]);
        assert_eq!(fixture.map.snow_depth, vec![0.0, 0.25, 0.5, 1.0]);
        assert_eq!(fixture.render.day_percent, Some(0.5));
        assert_eq!(fixture.render.shadow_vector.unwrap().x, 0.35);
        assert_eq!(fixture.render.glow_sources[0].radius, 6.0);
    }
}
