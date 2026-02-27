use crate::pawn::PawnFacing;

#[derive(Debug, Clone)]
pub struct ThingInstance {
    pub def_name: String,
    pub cell_x: i32,
    pub cell_z: i32,
}

#[derive(Debug, Clone)]
pub struct PawnInstance {
    pub label: String,
    pub cell_x: i32,
    pub cell_z: i32,
    pub facing: PawnFacing,
}

#[derive(Debug, Clone)]
pub struct FixtureMap {
    pub width: usize,
    pub height: usize,
    pub terrain: Vec<String>,
    pub things: Vec<ThingInstance>,
    pub pawns: Vec<PawnInstance>,
}

pub fn generate_fixture_map(
    width: usize,
    height: usize,
    terrain_defs: [&str; 3],
    thing_defs: &[String],
    pawn_count: usize,
) -> FixtureMap {
    let mut terrain = Vec::with_capacity(width * height);
    for z in 0..height {
        for x in 0..width {
            let patch = ((x / 8) + (z / 6)) % 3;
            terrain.push(terrain_defs[patch].to_string());
        }
    }

    let mut things = Vec::new();
    for (index, def_name) in thing_defs.iter().enumerate() {
        let center_x = (width / 2) as i32;
        let center_z = (height / 2) as i32;
        let (x, z) = if index == 0 {
            (center_x, center_z)
        } else {
            (
                (2 + ((index * 7) % width.saturating_sub(4).max(1))) as i32,
                (2 + ((index * 11) % height.saturating_sub(4).max(1))) as i32,
            )
        };
        things.push(ThingInstance {
            def_name: def_name.clone(),
            cell_x: x,
            cell_z: z,
        });
    }

    let facings = [
        PawnFacing::South,
        PawnFacing::East,
        PawnFacing::North,
        PawnFacing::West,
    ];
    let mut pawns = Vec::new();
    for index in 0..pawn_count {
        let center_x = (width / 2) as i32;
        let center_z = (height / 2) as i32;
        let (x, z) = if index == 0 {
            let anchor_x = if width > 2 {
                (center_x + 1).min(width as i32 - 1)
            } else {
                center_x
            };
            (anchor_x, center_z)
        } else {
            (
                (5 + ((index * 9) % width.saturating_sub(10).max(1))) as i32,
                (4 + ((index * 5) % height.saturating_sub(8).max(1))) as i32,
            )
        };
        pawns.push(PawnInstance {
            label: format!("Pawn{}", index + 1),
            cell_x: x,
            cell_z: z,
            facing: facings[index % facings.len()],
        });
    }

    FixtureMap {
        width,
        height,
        terrain,
        things,
        pawns,
    }
}
