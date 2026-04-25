use crate::fixtures::SceneFixture;
use glam::Vec2;

use super::{
    GlowSource, PathProgress, PawnState, RenderState, RoofTile, TerrainTile, ThingState, WorldState,
};

pub fn world_from_fixture(fixture: &SceneFixture) -> WorldState {
    let cell_count = fixture.map.width * fixture.map.height;
    let terrain = fixture
        .map
        .terrain
        .iter()
        .map(|tile| TerrainTile {
            terrain_def: tile.terrain_def.clone(),
        })
        .collect();

    let things: Vec<ThingState> = fixture
        .things
        .iter()
        .enumerate()
        .map(|(id, thing)| ThingState {
            id,
            def_name: thing.def_name.clone(),
            cell_x: thing.cell_x,
            cell_z: thing.cell_z,
            blocks_movement: thing.blocks_movement,
        })
        .collect();

    let mut thing_grid: Vec<Vec<usize>> = vec![Vec::new(); cell_count];
    for (thing_idx, thing) in things.iter().enumerate() {
        if thing.cell_x < 0 || thing.cell_z < 0 {
            continue;
        }
        let (x, z) = (thing.cell_x as usize, thing.cell_z as usize);
        if x >= fixture.map.width || z >= fixture.map.height {
            continue;
        }
        thing_grid[z * fixture.map.width + x].push(thing_idx);
    }

    let pawns = fixture
        .pawns
        .iter()
        .enumerate()
        .map(|(id, pawn)| PawnState {
            id,
            label: pawn
                .label
                .clone()
                .unwrap_or_else(|| format!("Pawn{}", id + 1)),
            cell_x: pawn.cell_x,
            cell_z: pawn.cell_z,
            facing: pawn.facing,
            body: pawn.body.clone(),
            head: pawn.head.clone(),
            hair: pawn.hair.clone(),
            beard: pawn.beard.clone(),
            apparel_defs: pawn.apparel_defs.clone(),
            world_pos: Vec2::new(pawn.cell_x as f32 + 0.5, pawn.cell_z as f32 + 0.5),
            path: PathProgress::Idle,
            move_speed_cells_per_sec: 3.0,
        })
        .collect();

    WorldState {
        width: fixture.map.width,
        height: fixture.map.height,
        terrain,
        render_state: RenderState {
            roofs: expand_roofs(fixture, cell_count),
            fog: expand_fog(fixture, cell_count),
            snow_depth: expand_snow_depth(fixture, cell_count),
            day_percent: fixture.render.day_percent,
            sky_glow: fixture.render.sky_glow.map(Into::into),
            shadow_color: fixture.render.shadow_color.map(Into::into),
            shadow_vector: fixture
                .render
                .shadow_vector
                .map(|vector| Vec2::new(vector.x, vector.z)),
            glow_sources: fixture
                .render
                .glow_sources
                .iter()
                .map(|source| GlowSource {
                    cell_x: source.cell_x,
                    cell_z: source.cell_z,
                    radius: source.radius,
                    color: source.color.into(),
                    overlight_radius: source.overlight_radius,
                })
                .collect(),
        },
        things,
        pawns,
        thing_grid,
    }
}

fn expand_roofs(fixture: &SceneFixture, cell_count: usize) -> Vec<RoofTile> {
    if fixture.map.roofs.is_empty() {
        return vec![RoofTile::default(); cell_count];
    }
    fixture
        .map
        .roofs
        .iter()
        .map(|roof| RoofTile {
            roofed: roof.roofed,
            thick: roof.thick,
        })
        .collect()
}

fn expand_fog(fixture: &SceneFixture, cell_count: usize) -> Vec<bool> {
    if fixture.map.fog.is_empty() {
        return vec![false; cell_count];
    }
    fixture.map.fog.clone()
}

fn expand_snow_depth(fixture: &SceneFixture, cell_count: usize) -> Vec<f32> {
    if fixture.map.snow_depth.is_empty() {
        return vec![0.0; cell_count];
    }
    fixture.map.snow_depth.clone()
}

#[cfg(test)]
mod tests {
    use crate::defs::RgbaColor;
    use crate::fixtures::{
        FixtureColor, GlowSourceSpec, MapSpec, RenderSpec, RoofCell, SceneFixture, TerrainCell,
    };

    use super::world_from_fixture;

    #[test]
    fn fixture_render_state_defaults_to_map_sized_empty_grids() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 3,
                height: 2,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string(),
                    };
                    6
                ],
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let render = world.render_state();
        assert_eq!(render.roofs.len(), 6);
        assert!(render.roofs.iter().all(|roof| !roof.roofed && !roof.thick));
        assert_eq!(render.fog, vec![false; 6]);
        assert_eq!(render.snow_depth, vec![0.0; 6]);
        assert!(render.glow_sources.is_empty());
    }

    #[test]
    fn fixture_render_state_carries_explicit_grids_and_light() {
        let world = world_from_fixture(&SceneFixture {
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
                roofs: vec![
                    RoofCell::default(),
                    RoofCell {
                        roofed: true,
                        thick: false,
                    },
                    RoofCell {
                        roofed: true,
                        thick: true,
                    },
                    RoofCell::default(),
                ],
                fog: vec![false, true, true, false],
                snow_depth: vec![0.0, 0.25, 0.5, 1.0],
            },
            render: RenderSpec {
                day_percent: Some(0.33),
                sky_glow: Some(FixtureColor {
                    r: 0.6,
                    g: 0.7,
                    b: 0.8,
                    a: 1.0,
                }),
                shadow_color: Some(FixtureColor {
                    r: 0.1,
                    g: 0.2,
                    b: 0.3,
                    a: 0.4,
                }),
                shadow_vector: None,
                glow_sources: vec![GlowSourceSpec {
                    cell_x: 1,
                    cell_z: 0,
                    radius: 5.5,
                    color: FixtureColor {
                        r: 255.0,
                        g: 220.0,
                        b: 120.0,
                        a: 0.0,
                    },
                    overlight_radius: 2.0,
                }],
            },
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let render = world.render_state();
        assert!(render.roofs[1].roofed);
        assert!(render.roofs[2].thick);
        assert_eq!(render.fog, vec![false, true, true, false]);
        assert_eq!(render.snow_depth, vec![0.0, 0.25, 0.5, 1.0]);
        assert_eq!(render.day_percent, Some(0.33));
        assert_eq!(
            render.sky_glow,
            Some(RgbaColor {
                r: 0.6,
                g: 0.7,
                b: 0.8,
                a: 1.0,
            })
        );
        assert_eq!(render.glow_sources[0].cell_x, 1);
        assert_eq!(render.glow_sources[0].radius, 5.5);
    }
}
