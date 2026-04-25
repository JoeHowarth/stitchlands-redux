use std::collections::HashMap;

use crate::cell::Cell;
use crate::defs::{GlowerProps, RgbaColor, ThingDef};
use crate::world::{GlowSource, WorldState};

#[derive(Debug, Clone, PartialEq)]
pub struct GlowGrid {
    width: usize,
    height: usize,
    visual_glow: Vec<f32>,
}

impl GlowGrid {
    pub fn from_world(thing_defs: &HashMap<String, ThingDef>, world: &WorldState) -> Self {
        let width = world.width();
        let height = world.height();
        let mut visual_glow = Vec::with_capacity(width * height);

        for z in 0..height {
            for x in 0..width {
                let cell = Cell::new(x as i32, z as i32);
                visual_glow.push(artificial_glow_brightness(thing_defs, world, cell));
            }
        }

        Self {
            width,
            height,
            visual_glow,
        }
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
        if cell.x < 0 || cell.z < 0 {
            return 0.0;
        }
        let x = cell.x as usize;
        let z = cell.z as usize;
        if x >= self.width || z >= self.height {
            return 0.0;
        }
        self.visual_glow[z * self.width + x]
    }
}

fn artificial_glow_brightness(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
    cell: Cell,
) -> f32 {
    let fixture_brightness = world
        .render_state()
        .glow_sources
        .iter()
        .map(|source| glow_source_brightness(source, cell))
        .fold(0.0, f32::max);

    let glower_brightness = world
        .things()
        .iter()
        .filter_map(|thing| {
            let glower = thing_defs.get(&thing.def_name).and_then(|def| def.glower)?;
            Some(glower_brightness(thing.cell_x, thing.cell_z, glower, cell))
        })
        .fold(0.0, f32::max);

    fixture_brightness.max(glower_brightness)
}

fn glow_source_brightness(source: &GlowSource, cell: Cell) -> f32 {
    point_glow_brightness(
        source.cell_x,
        source.cell_z,
        source.radius,
        source.overlight_radius,
        source.color,
        cell,
    )
}

fn glower_brightness(cell_x: i32, cell_z: i32, glower: GlowerProps, cell: Cell) -> f32 {
    point_glow_brightness(
        cell_x,
        cell_z,
        glower.glow_radius,
        glower.overlight_radius,
        glower.glow_color,
        cell,
    )
}

fn point_glow_brightness(
    source_x: i32,
    source_z: i32,
    radius: f32,
    overlight_radius: f32,
    color: RgbaColor,
    cell: Cell,
) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    let dx = cell.x as f32 + 0.5 - (source_x as f32 + 0.5);
    let dz = cell.z as f32 + 0.5 - (source_z as f32 + 0.5);
    let distance = (dx * dx + dz * dz).sqrt();
    let falloff = if overlight_radius > 0.0 && distance <= overlight_radius {
        1.0
    } else {
        (1.0 - distance / radius).clamp(0.0, 1.0)
    };
    falloff * color_brightness(color)
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

    fn thing_def(def_name: &str) -> ThingDef {
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
            block_light: false,
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
        let mut lamp = thing_def("Lamp");
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
