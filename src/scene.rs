use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ThingInstance {
    pub def_name: String,
    pub cell_x: i32,
    pub cell_z: i32,
}

#[derive(Debug, Clone, Copy)]
pub enum PawnFacing {
    North,
    East,
    South,
    West,
}

#[derive(Debug, Clone)]
pub struct PawnInstance {
    pub label: String,
    pub tex_path: String,
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

impl FixtureMap {
    pub fn terrain_at(&self, x: usize, z: usize) -> &str {
        &self.terrain[z * self.width + x]
    }
}

pub fn generate_fixture_map(
    width: usize,
    height: usize,
    terrain_defs: [&str; 3],
    thing_defs: &[String],
    pawn_tex_paths: &[String],
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
        let x = 2 + ((index * 7) % width.saturating_sub(4).max(1));
        let z = 2 + ((index * 11) % height.saturating_sub(4).max(1));
        things.push(ThingInstance {
            def_name: def_name.clone(),
            cell_x: x as i32,
            cell_z: z as i32,
        });
    }

    let facings = [
        PawnFacing::South,
        PawnFacing::East,
        PawnFacing::North,
        PawnFacing::West,
    ];
    let mut pawns = Vec::new();
    for (index, tex_path) in pawn_tex_paths.iter().enumerate() {
        let x = 5 + ((index * 9) % width.saturating_sub(10).max(1));
        let z = 4 + ((index * 5) % height.saturating_sub(8).max(1));
        pawns.push(PawnInstance {
            label: format!("Pawn{}", index + 1),
            tex_path: tex_path.clone(),
            cell_x: x as i32,
            cell_z: z as i32,
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

pub fn sorted_things_by_altitude(things: &[ThingInstance]) -> Vec<ThingInstance> {
    let mut out = things.to_vec();
    out.sort_by(|a, b| {
        a.cell_z
            .cmp(&b.cell_z)
            .then(a.cell_x.cmp(&b.cell_x))
            .then(a.def_name.cmp(&b.def_name))
    });
    out
}

pub fn sorted_pawns(pawns: &[PawnInstance]) -> Vec<PawnInstance> {
    let mut out = pawns.to_vec();
    out.sort_by(|a, b| {
        a.cell_z
            .cmp(&b.cell_z)
            .then(a.cell_x.cmp(&b.cell_x))
            .then(a.label.cmp(&b.label))
    });
    out
}

pub fn count_terrain_families(map: &FixtureMap) -> usize {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for terrain in &map.terrain {
        *counts.entry(terrain).or_default() += 1;
    }
    counts.len()
}
