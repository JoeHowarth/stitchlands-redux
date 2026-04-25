use std::collections::{HashMap, VecDeque};

use crate::cell::Cell;
use crate::defs::{GlowerProps, RgbaColor, ThingDef};
use crate::world::{GlowSource, WorldState};

const CARDINAL_STEP_COST: f32 = 1.0;
const DIAGONAL_STEP_COST: f32 = std::f32::consts::SQRT_2;
const PROPAGATION_EPSILON: f32 = 0.0001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowSample {
    pub intensity: f32,
    pub color: RgbaColor,
}

impl Default for GlowSample {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            color: RgbaColor::WHITE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GlowEmitter {
    cell: Cell,
    radius: f32,
    overlight_radius: f32,
    color: RgbaColor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlowGrid {
    width: usize,
    height: usize,
    visual_glow: Vec<GlowSample>,
}

impl GlowGrid {
    pub fn from_world(thing_defs: &HashMap<String, ThingDef>, world: &WorldState) -> Self {
        let width = world.width();
        let height = world.height();
        let blockers = blocker_grid(thing_defs, world);
        let mut grid = Self {
            width,
            height,
            visual_glow: vec![GlowSample::default(); width * height],
        };

        for emitter in glow_emitters(thing_defs, world) {
            grid.propagate_emitter(emitter, &blockers);
        }

        grid
    }

    pub fn has_inputs(thing_defs: &HashMap<String, ThingDef>, world: &WorldState) -> bool {
        !world.render_state().glow_sources.is_empty()
            || world.things().iter().any(|thing| {
                thing_defs
                    .get(&thing.def_name)
                    .and_then(|def| def.glower)
                    .is_some()
            })
    }

    pub fn visual_glow_at(&self, cell: Cell) -> f32 {
        self.visual_glow_sample_at(cell).intensity
    }

    pub fn visual_glow_sample_at(&self, cell: Cell) -> GlowSample {
        if cell.x < 0 || cell.z < 0 {
            return GlowSample::default();
        }
        let x = cell.x as usize;
        let z = cell.z as usize;
        if x >= self.width || z >= self.height {
            return GlowSample::default();
        }
        self.visual_glow[z * self.width + x]
    }

    fn propagate_emitter(&mut self, emitter: GlowEmitter, blockers: &[bool]) {
        if emitter.radius <= 0.0 || !self.contains(emitter.cell) {
            return;
        }

        let cell_count = self.width * self.height;
        let mut distances = vec![f32::INFINITY; cell_count];
        let source_index = self.cell_index(emitter.cell).expect("source is in bounds");
        distances[source_index] = 0.0;

        let mut queue = VecDeque::from([emitter.cell]);
        while let Some(cell) = queue.pop_front() {
            let Some(cell_index) = self.cell_index(cell) else {
                continue;
            };
            let distance = distances[cell_index];
            self.apply_emitter_sample(cell_index, distance, emitter);

            if blockers[cell_index] && cell != emitter.cell {
                continue;
            }

            for (dx, dz, step_cost) in neighbors() {
                let next = Cell::new(cell.x + dx, cell.z + dz);
                let Some(next_index) = self.cell_index(next) else {
                    continue;
                };
                let next_distance = distance + step_cost;
                if next_distance > emitter.radius {
                    continue;
                }
                if next_distance + PROPAGATION_EPSILON < distances[next_index] {
                    distances[next_index] = next_distance;
                    queue.push_back(next);
                }
            }
        }
    }

    fn apply_emitter_sample(&mut self, cell_index: usize, distance: f32, emitter: GlowEmitter) {
        let intensity = emitter_intensity(emitter, distance);
        if intensity > self.visual_glow[cell_index].intensity {
            self.visual_glow[cell_index] = GlowSample {
                intensity,
                color: emitter.color,
            };
        }
    }

    fn contains(&self, cell: Cell) -> bool {
        self.cell_index(cell).is_some()
    }

    fn cell_index(&self, cell: Cell) -> Option<usize> {
        if cell.x < 0 || cell.z < 0 {
            return None;
        }
        let x = cell.x as usize;
        let z = cell.z as usize;
        (x < self.width && z < self.height).then_some(z * self.width + x)
    }
}

fn blocker_grid(thing_defs: &HashMap<String, ThingDef>, world: &WorldState) -> Vec<bool> {
    let mut blockers = vec![false; world.width() * world.height()];
    for thing in world.things() {
        let Some(def) = thing_defs.get(&thing.def_name) else {
            continue;
        };
        if !def.block_light {
            continue;
        }
        let cell = Cell::new(thing.cell_x, thing.cell_z);
        if cell.x < 0 || cell.z < 0 {
            continue;
        }
        let x = cell.x as usize;
        let z = cell.z as usize;
        if x < world.width() && z < world.height() {
            blockers[z * world.width() + x] = true;
        }
    }
    blockers
}

fn glow_emitters(thing_defs: &HashMap<String, ThingDef>, world: &WorldState) -> Vec<GlowEmitter> {
    let mut emitters: Vec<_> = world
        .render_state()
        .glow_sources
        .iter()
        .map(glow_source_emitter)
        .collect();
    emitters.extend(world.things().iter().filter_map(|thing| {
        let glower = thing_defs.get(&thing.def_name).and_then(|def| def.glower)?;
        Some(glower_emitter(thing.cell_x, thing.cell_z, glower))
    }));
    emitters
}

fn glow_source_emitter(source: &GlowSource) -> GlowEmitter {
    GlowEmitter {
        cell: Cell::new(source.cell_x, source.cell_z),
        radius: source.radius,
        overlight_radius: source.overlight_radius,
        color: source.color,
    }
}

fn glower_emitter(cell_x: i32, cell_z: i32, glower: GlowerProps) -> GlowEmitter {
    GlowEmitter {
        cell: Cell::new(cell_x, cell_z),
        radius: glower.glow_radius,
        overlight_radius: glower.overlight_radius,
        color: glower.glow_color,
    }
}

fn emitter_intensity(emitter: GlowEmitter, distance: f32) -> f32 {
    let falloff = if emitter.overlight_radius > 0.0 && distance <= emitter.overlight_radius {
        1.0
    } else {
        (1.0 - distance / emitter.radius).clamp(0.0, 1.0)
    };
    falloff * color_brightness(emitter.color)
}

fn neighbors() -> [(i32, i32, f32); 8] {
    [
        (-1, 0, CARDINAL_STEP_COST),
        (1, 0, CARDINAL_STEP_COST),
        (0, -1, CARDINAL_STEP_COST),
        (0, 1, CARDINAL_STEP_COST),
        (-1, -1, DIAGONAL_STEP_COST),
        (1, -1, DIAGONAL_STEP_COST),
        (-1, 1, DIAGONAL_STEP_COST),
        (1, 1, DIAGONAL_STEP_COST),
    ]
}

fn color_brightness(color: RgbaColor) -> f32 {
    let average = (color.r + color.g + color.b) / 3.0;
    let max_component = color.r.max(color.g).max(color.b);
    if max_component > 1.0 {
        (average / 255.0).clamp(0.0, 1.0)
    } else {
        average.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use glam::{Vec2, Vec3};

    use crate::cell::Cell;
    use crate::defs::{GlowerProps, GraphicData, GraphicKind, RgbaColor, ThingDef};
    use crate::fixtures::{
        FixtureColor, GlowSourceSpec, MapSpec, RenderSpec, SceneFixture, TerrainCell, ThingSpawn,
    };
    use crate::linking::{LinkDrawerType, LinkFlags};
    use crate::world::world_from_fixture;

    use super::GlowGrid;

    fn soil(count: usize) -> Vec<TerrainCell> {
        vec![
            TerrainCell {
                terrain_def: "Soil".to_string(),
            };
            count
        ]
    }

    fn thing_def(def_name: &str, block_light: bool) -> ThingDef {
        ThingDef {
            def_name: def_name.to_string(),
            graphic_data: GraphicData {
                tex_path: "Things/Test".to_string(),
                kind: GraphicKind::Single,
                color: RgbaColor::WHITE,
                draw_size: Vec2::ONE,
                draw_offset: Vec3::ZERO,
                shadow_data: None,
                link_type: LinkDrawerType::None,
                link_flags: LinkFlags::EMPTY,
            },
            block_light,
            holds_roof: false,
            cast_edge_shadows: false,
            static_sun_shadow_height: 0.0,
            glower: None,
        }
    }

    #[test]
    fn fixture_glow_source_brightens_nearby_cells() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 3,
                height: 1,
                terrain: soil(3),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                glow_sources: vec![GlowSourceSpec {
                    cell_x: 0,
                    cell_z: 0,
                    radius: 2.0,
                    color: FixtureColor {
                        r: 255.0,
                        g: 255.0,
                        b: 255.0,
                        a: 0.0,
                    },
                    overlight_radius: 0.0,
                }],
                ..RenderSpec::default()
            },
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let glow_grid = GlowGrid::from_world(&HashMap::new(), &world);

        assert!(GlowGrid::has_inputs(&HashMap::new(), &world));
        assert!(
            glow_grid.visual_glow_at(Cell::new(0, 0)) > glow_grid.visual_glow_at(Cell::new(2, 0))
        );
    }

    #[test]
    fn thing_glower_brightens_nearby_cells() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 3,
                height: 1,
                terrain: soil(3),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: vec![ThingSpawn {
                def_name: "Lamp".to_string(),
                cell_x: 0,
                cell_z: 0,
                blocks_movement: false,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let mut lamp = thing_def("Lamp", false);
        lamp.glower = Some(GlowerProps {
            glow_radius: 1.5,
            glow_color: RgbaColor {
                r: 255.0,
                g: 255.0,
                b: 255.0,
                a: 0.0,
            },
            overlight_radius: 0.0,
        });
        let thing_defs = HashMap::from([("Lamp".to_string(), lamp)]);

        let glow_grid = GlowGrid::from_world(&thing_defs, &world);

        assert!(GlowGrid::has_inputs(&thing_defs, &world));
        assert!(
            glow_grid.visual_glow_at(Cell::new(0, 0)) > glow_grid.visual_glow_at(Cell::new(2, 0))
        );
    }

    #[test]
    fn block_light_cell_stops_glow_propagation() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 4,
                height: 1,
                terrain: soil(4),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                glow_sources: vec![GlowSourceSpec {
                    cell_x: 0,
                    cell_z: 0,
                    radius: 4.0,
                    color: FixtureColor {
                        r: 255.0,
                        g: 255.0,
                        b: 255.0,
                        a: 0.0,
                    },
                    overlight_radius: 0.0,
                }],
                ..RenderSpec::default()
            },
            things: vec![ThingSpawn {
                def_name: "Wall".to_string(),
                cell_x: 1,
                cell_z: 0,
                blocks_movement: true,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let thing_defs = HashMap::from([("Wall".to_string(), thing_def("Wall", true))]);

        let glow_grid = GlowGrid::from_world(&thing_defs, &world);

        assert!(glow_grid.visual_glow_at(Cell::new(1, 0)) > 0.0);
        assert_eq!(glow_grid.visual_glow_at(Cell::new(2, 0)), 0.0);
        assert_eq!(glow_grid.visual_glow_at(Cell::new(3, 0)), 0.0);
    }

    #[test]
    fn movement_blocking_without_block_light_does_not_stop_glow() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 4,
                height: 1,
                terrain: soil(4),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                glow_sources: vec![GlowSourceSpec {
                    cell_x: 0,
                    cell_z: 0,
                    radius: 4.0,
                    color: FixtureColor {
                        r: 255.0,
                        g: 255.0,
                        b: 255.0,
                        a: 0.0,
                    },
                    overlight_radius: 0.0,
                }],
                ..RenderSpec::default()
            },
            things: vec![ThingSpawn {
                def_name: "Crate".to_string(),
                cell_x: 1,
                cell_z: 0,
                blocks_movement: true,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let thing_defs = HashMap::from([("Crate".to_string(), thing_def("Crate", false))]);

        let glow_grid = GlowGrid::from_world(&thing_defs, &world);

        assert!(glow_grid.visual_glow_at(Cell::new(2, 0)) > 0.0);
        assert!(glow_grid.visual_glow_at(Cell::new(3, 0)) > 0.0);
    }

    #[test]
    fn diagonal_steps_use_longer_cost_for_radius_cutoff() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 2,
                height: 2,
                terrain: soil(4),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                glow_sources: vec![GlowSourceSpec {
                    cell_x: 0,
                    cell_z: 0,
                    radius: 1.1,
                    color: FixtureColor {
                        r: 255.0,
                        g: 255.0,
                        b: 255.0,
                        a: 0.0,
                    },
                    overlight_radius: 0.0,
                }],
                ..RenderSpec::default()
            },
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let glow_grid = GlowGrid::from_world(&HashMap::new(), &world);

        assert!(glow_grid.visual_glow_at(Cell::new(1, 0)) > 0.0);
        assert!(glow_grid.visual_glow_at(Cell::new(0, 1)) > 0.0);
        assert_eq!(glow_grid.visual_glow_at(Cell::new(1, 1)), 0.0);
    }

    #[test]
    fn source_cell_can_be_block_light_and_still_emit() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 3,
                height: 1,
                terrain: soil(3),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: vec![ThingSpawn {
                def_name: "GlowWall".to_string(),
                cell_x: 0,
                cell_z: 0,
                blocks_movement: true,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let mut glow_wall = thing_def("GlowWall", true);
        glow_wall.glower = Some(GlowerProps {
            glow_radius: 3.0,
            glow_color: RgbaColor {
                r: 255.0,
                g: 255.0,
                b: 255.0,
                a: 0.0,
            },
            overlight_radius: 0.0,
        });
        let thing_defs = HashMap::from([("GlowWall".to_string(), glow_wall)]);

        let glow_grid = GlowGrid::from_world(&thing_defs, &world);

        assert!(glow_grid.visual_glow_at(Cell::new(0, 0)) > 0.0);
        assert!(glow_grid.visual_glow_at(Cell::new(1, 0)) > 0.0);
        assert!(glow_grid.visual_glow_at(Cell::new(2, 0)) > 0.0);
    }

    #[test]
    fn carries_color_for_strongest_glow_sample() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 3,
                height: 1,
                terrain: soil(3),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                glow_sources: vec![
                    GlowSourceSpec {
                        cell_x: 0,
                        cell_z: 0,
                        radius: 3.0,
                        color: FixtureColor {
                            r: 255.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        },
                        overlight_radius: 0.0,
                    },
                    GlowSourceSpec {
                        cell_x: 2,
                        cell_z: 0,
                        radius: 3.0,
                        color: FixtureColor {
                            r: 0.0,
                            g: 255.0,
                            b: 0.0,
                            a: 0.0,
                        },
                        overlight_radius: 1.0,
                    },
                ],
                ..RenderSpec::default()
            },
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let glow_grid = GlowGrid::from_world(&HashMap::new(), &world);
        let sample = glow_grid.visual_glow_sample_at(Cell::new(2, 0));

        assert!(sample.intensity > 0.0);
        assert_eq!(sample.color.g, 255.0);
    }

    #[test]
    fn sky_inputs_are_not_visual_glow_inputs() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 1,
                height: 1,
                terrain: soil(1),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                day_percent: Some(0.5),
                sky_glow: Some(FixtureColor {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                }),
                ..RenderSpec::default()
            },
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let glow_grid = GlowGrid::from_world(&HashMap::new(), &world);

        assert!(!GlowGrid::has_inputs(&HashMap::new(), &world));
        assert_eq!(glow_grid.visual_glow_at(Cell::new(0, 0)), 0.0);
    }
}
