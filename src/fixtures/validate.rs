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

    for (idx, tile) in fixture.map.terrain.iter().enumerate() {
        if tile.terrain_def.trim().is_empty() {
            errors.push(format!("map terrain[{}].terrain_def is empty", idx));
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
            super::schema::PawnFacingSpec::North
            | super::schema::PawnFacingSpec::East
            | super::schema::PawnFacingSpec::South
            | super::schema::PawnFacingSpec::West => {}
        }
    }

    if let Some(camera) = &fixture.camera {
        if !(camera.zoom.is_finite() && camera.zoom > 0.0) {
            errors.push("camera.zoom must be finite and > 0".to_string());
        }
        if !camera.center_x.is_finite() || !camera.center_z.is_finite() {
            errors.push("camera center must be finite".to_string());
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
