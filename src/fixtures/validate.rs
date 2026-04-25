use anyhow::{Result, bail};

use super::schema::SceneFixture;

const SUPPORTED_SCHEMA_VERSION: u32 = 2;

pub fn validate_fixture(fixture: &SceneFixture) -> Result<()> {
    let mut errors = Vec::new();

    if fixture.schema_version != SUPPORTED_SCHEMA_VERSION {
        errors.push(format!(
            "unsupported schema_version {} (expected {})",
            fixture.schema_version, SUPPORTED_SCHEMA_VERSION
        ));
    }

    if fixture.map.width == 0 || fixture.map.height == 0 {
        errors.push("map dimensions must be > 0".to_string());
    }

    let expected_cells = fixture.map.width.saturating_mul(fixture.map.height);
    if fixture.map.terrain.len() != expected_cells {
        errors.push(format!(
            "map terrain cell count mismatch: got {} expected {} ({}x{})",
            fixture.map.terrain.len(),
            expected_cells,
            fixture.map.width,
            fixture.map.height
        ));
    }
    validate_grid_len(
        "map.roofs",
        fixture.map.roofs.len(),
        expected_cells,
        &mut errors,
    );
    validate_grid_len(
        "map.fog",
        fixture.map.fog.len(),
        expected_cells,
        &mut errors,
    );
    validate_grid_len(
        "map.snow_depth",
        fixture.map.snow_depth.len(),
        expected_cells,
        &mut errors,
    );

    for (idx, tile) in fixture.map.terrain.iter().enumerate() {
        if tile.terrain_def.trim().is_empty() {
            errors.push(format!("map terrain[{}].terrain_def is empty", idx));
        }
    }
    for (idx, roof) in fixture.map.roofs.iter().enumerate() {
        if roof.thick && !roof.roofed {
            errors.push(format!("map.roofs[{idx}] cannot be thick without roofed"));
        }
    }
    for (idx, depth) in fixture.map.snow_depth.iter().enumerate() {
        if !depth.is_finite() || !(0.0..=1.0).contains(depth) {
            errors.push(format!(
                "map.snow_depth[{idx}] must be finite and between 0.0 and 1.0"
            ));
        }
    }

    for (idx, thing) in fixture.things.iter().enumerate() {
        if thing.def_name.trim().is_empty() {
            errors.push(format!("things[{}].def_name is empty", idx));
        }
        if !thing.blocks_movement && thing.def_name.trim().is_empty() {
            errors.push(format!(
                "things[{}] non-blocking item must still provide a def_name",
                idx
            ));
        }
        if !in_bounds(
            thing.cell_x,
            thing.cell_z,
            fixture.map.width,
            fixture.map.height,
        ) {
            errors.push(format!(
                "things[{}] cell ({}, {}) is out of map bounds {}x{}",
                idx, thing.cell_x, thing.cell_z, fixture.map.width, fixture.map.height
            ));
        }
    }

    for (idx, pawn) in fixture.pawns.iter().enumerate() {
        if !in_bounds(
            pawn.cell_x,
            pawn.cell_z,
            fixture.map.width,
            fixture.map.height,
        ) {
            errors.push(format!(
                "pawns[{}] cell ({}, {}) is out of map bounds {}x{}",
                idx, pawn.cell_x, pawn.cell_z, fixture.map.width, fixture.map.height
            ));
        }
        if let Some(label) = &pawn.label
            && label.trim().is_empty()
        {
            errors.push(format!(
                "pawns[{}].label cannot be blank when provided",
                idx
            ));
        }
        for (name, value) in [
            ("body", pawn.body.as_deref()),
            ("head", pawn.head.as_deref()),
            ("hair", pawn.hair.as_deref()),
            ("beard", pawn.beard.as_deref()),
        ] {
            if let Some(value) = value
                && value.trim().is_empty()
            {
                errors.push(format!("pawns[{}].{} cannot be blank", idx, name));
            }
        }
        for (apparel_idx, def_name) in pawn.apparel_defs.iter().enumerate() {
            if def_name.trim().is_empty() {
                errors.push(format!(
                    "pawns[{}].apparel_defs[{}] cannot be blank",
                    idx, apparel_idx
                ));
            }
        }
        match pawn.facing {
            crate::pawn::PawnFacing::North
            | crate::pawn::PawnFacing::East
            | crate::pawn::PawnFacing::South
            | crate::pawn::PawnFacing::West => {}
        }
    }

    if let Some(camera) = &fixture.camera
        && (!camera.center_x.is_finite() || !camera.center_z.is_finite())
    {
        errors.push("camera center must be finite".to_string());
    }

    if let Some(day_percent) = fixture.render.day_percent
        && (!day_percent.is_finite() || !(0.0..=1.0).contains(&day_percent))
    {
        errors.push("render.day_percent must be finite and between 0.0 and 1.0".to_string());
    }
    if let Some(color) = fixture.render.sky_glow
        && !color_is_finite(color)
    {
        errors.push("render.sky_glow components must be finite".to_string());
    }
    if let Some(color) = fixture.render.shadow_color
        && !color_is_finite(color)
    {
        errors.push("render.shadow_color components must be finite".to_string());
    }
    if let Some(vector) = fixture.render.shadow_vector
        && (!vector.x.is_finite() || !vector.z.is_finite())
    {
        errors.push("render.shadow_vector components must be finite".to_string());
    }
    for (idx, source) in fixture.render.glow_sources.iter().enumerate() {
        if !in_bounds(
            source.cell_x,
            source.cell_z,
            fixture.map.width,
            fixture.map.height,
        ) {
            errors.push(format!(
                "render.glow_sources[{idx}] cell ({}, {}) is out of map bounds {}x{}",
                source.cell_x, source.cell_z, fixture.map.width, fixture.map.height
            ));
        }
        if !source.radius.is_finite() || source.radius < 0.0 {
            errors.push(format!(
                "render.glow_sources[{idx}].radius must be finite and >= 0.0"
            ));
        }
        if !source.overlight_radius.is_finite() || source.overlight_radius < 0.0 {
            errors.push(format!(
                "render.glow_sources[{idx}].overlight_radius must be finite and >= 0.0"
            ));
        }
        if !color_is_finite(source.color) {
            errors.push(format!(
                "render.glow_sources[{idx}].color components must be finite"
            ));
        }
    }

    if errors.is_empty() {
        return Ok(());
    }

    let mut details = String::new();
    for err in errors {
        details.push_str("- ");
        details.push_str(&err);
        details.push('\n');
    }
    bail!("fixture validation failed:\n{details}");
}

fn in_bounds(x: i32, z: i32, width: usize, height: usize) -> bool {
    x >= 0 && z >= 0 && x < width as i32 && z < height as i32
}

fn validate_grid_len(name: &str, len: usize, expected_cells: usize, errors: &mut Vec<String>) {
    if len != 0 && len != expected_cells {
        errors.push(format!(
            "{name} cell count mismatch: got {len} expected {expected_cells}"
        ));
    }
}

fn color_is_finite(color: super::schema::FixtureColor) -> bool {
    color.r.is_finite() && color.g.is_finite() && color.b.is_finite() && color.a.is_finite()
}

#[cfg(test)]
mod tests {
    use super::validate_fixture;
    use crate::fixtures::{
        FixtureColor, GlowSourceSpec, MapSpec, RenderSpec, RoofCell, SceneFixture, TerrainCell,
    };

    fn base_fixture() -> SceneFixture {
        SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 2,
                height: 2,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string(),
                    };
                    4
                ],
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        }
    }

    #[test]
    fn validates_render_state_grid_lengths() {
        let mut fixture = base_fixture();
        fixture.map.fog = vec![true, false];

        let err = validate_fixture(&fixture).unwrap_err().to_string();

        assert!(err.contains("map.fog cell count mismatch"));
    }

    #[test]
    fn validates_roof_snow_and_glow_source_values() {
        let mut fixture = base_fixture();
        fixture.map.roofs = vec![
            RoofCell::default(),
            RoofCell {
                roofed: false,
                thick: true,
            },
            RoofCell::default(),
            RoofCell::default(),
        ];
        fixture.map.snow_depth = vec![0.0, 0.25, 1.25, 0.0];
        fixture.render = RenderSpec {
            day_percent: Some(1.2),
            sky_glow: None,
            shadow_color: None,
            shadow_vector: None,
            glow_sources: vec![GlowSourceSpec {
                cell_x: 3,
                cell_z: 0,
                radius: -1.0,
                color: FixtureColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                overlight_radius: 0.0,
            }],
        };

        let err = validate_fixture(&fixture).unwrap_err().to_string();

        assert!(err.contains("cannot be thick without roofed"));
        assert!(err.contains("map.snow_depth[2]"));
        assert!(err.contains("render.day_percent"));
        assert!(err.contains("render.glow_sources[0] cell"));
        assert!(err.contains("render.glow_sources[0].radius"));
    }
}
