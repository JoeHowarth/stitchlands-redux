use std::collections::HashMap;

use glam::Vec2;

use crate::cell::Cell;
use crate::defs::{RgbaColor, ThingDef};
use crate::renderer::{ColoredMeshInput, ColoredVertex, OverlayPass};
use crate::world::{ThingState, WorldState};

const SHADOW_OVERLAY_DEPTH: f32 = -0.18;
const EDGE_SHADOW_IN_DIST: f32 = 0.45;
const EDGE_SHADOW_ALPHA: f32 = (255.0 - 195.0) / 255.0;
const DEFAULT_SHADOW_VECTOR: Vec2 = Vec2::new(0.45, -0.35);

pub fn build_shadow_overlays(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Vec<ColoredMeshInput> {
    let mut overlays = Vec::new();
    if let Some(sun) = build_sun_shadow_overlay(thing_defs, world) {
        overlays.push(sun);
    }
    if let Some(edge) = build_edge_shadow_overlay(thing_defs, world) {
        overlays.push(edge);
    }
    overlays
}

fn build_sun_shadow_overlay(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Option<ColoredMeshInput> {
    let shadow_vector = world
        .render_state()
        .shadow_vector
        .unwrap_or(DEFAULT_SHADOW_VECTOR);
    let shadow_color = world.render_state().shadow_color.unwrap_or(RgbaColor {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.55,
    });
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for thing in world.things() {
        let Some(def) = thing_defs.get(&thing.def_name) else {
            continue;
        };
        let height = def.static_sun_shadow_height;
        if height <= 0.0 {
            continue;
        }
        let offset = shadow_vector * height.max(0.0);
        let alpha = (shadow_color.a * height).clamp(0.0, 1.0);
        push_solid_quad(
            &mut vertices,
            &mut indices,
            thing.cell_x as f32 + offset.x,
            thing.cell_z as f32 + offset.y,
            thing.cell_x as f32 + 1.0 + offset.x,
            thing.cell_z as f32 + 1.0 + offset.y,
            color_with_alpha(shadow_color, alpha),
        );
    }

    mesh_if_not_empty(vertices, indices)
}

fn build_edge_shadow_overlay(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Option<ColoredMeshInput> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for z in 0..world.height() {
        for x in 0..world.width() {
            let cell = Cell::new(x as i32, z as i32);
            if cell_casts_edge_shadow(thing_defs, world, cell) {
                push_solid_quad(
                    &mut vertices,
                    &mut indices,
                    x as f32,
                    z as f32,
                    x as f32 + 1.0,
                    z as f32 + 1.0,
                    [0.0, 0.0, 0.0, EDGE_SHADOW_ALPHA],
                );
                continue;
            }

            if neighbor_casts_edge_shadow(thing_defs, world, cell, 0, 1) {
                push_gradient_quad(
                    &mut vertices,
                    &mut indices,
                    [
                        (x as f32, z as f32 + 1.0, EDGE_SHADOW_ALPHA),
                        (x as f32 + 1.0, z as f32 + 1.0, EDGE_SHADOW_ALPHA),
                        (x as f32 + 1.0, z as f32 + 1.0 - EDGE_SHADOW_IN_DIST, 0.0),
                        (x as f32, z as f32 + 1.0 - EDGE_SHADOW_IN_DIST, 0.0),
                    ],
                );
            }
            if neighbor_casts_edge_shadow(thing_defs, world, cell, 1, 0) {
                push_gradient_quad(
                    &mut vertices,
                    &mut indices,
                    [
                        (x as f32 + 1.0, z as f32, EDGE_SHADOW_ALPHA),
                        (x as f32 + 1.0, z as f32 + 1.0, EDGE_SHADOW_ALPHA),
                        (x as f32 + 1.0 - EDGE_SHADOW_IN_DIST, z as f32 + 1.0, 0.0),
                        (x as f32 + 1.0 - EDGE_SHADOW_IN_DIST, z as f32, 0.0),
                    ],
                );
            }
            if neighbor_casts_edge_shadow(thing_defs, world, cell, 0, -1) {
                push_gradient_quad(
                    &mut vertices,
                    &mut indices,
                    [
                        (x as f32, z as f32, EDGE_SHADOW_ALPHA),
                        (x as f32 + 1.0, z as f32, EDGE_SHADOW_ALPHA),
                        (x as f32 + 1.0, z as f32 + EDGE_SHADOW_IN_DIST, 0.0),
                        (x as f32, z as f32 + EDGE_SHADOW_IN_DIST, 0.0),
                    ],
                );
            }
            if neighbor_casts_edge_shadow(thing_defs, world, cell, -1, 0) {
                push_gradient_quad(
                    &mut vertices,
                    &mut indices,
                    [
                        (x as f32, z as f32, EDGE_SHADOW_ALPHA),
                        (x as f32, z as f32 + 1.0, EDGE_SHADOW_ALPHA),
                        (x as f32 + EDGE_SHADOW_IN_DIST, z as f32 + 1.0, 0.0),
                        (x as f32 + EDGE_SHADOW_IN_DIST, z as f32, 0.0),
                    ],
                );
            }
        }
    }

    mesh_if_not_empty(vertices, indices)
}

fn push_solid_quad(
    vertices: &mut Vec<ColoredVertex>,
    indices: &mut Vec<u32>,
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    color: [f32; 4],
) {
    push_gradient_quad(
        vertices,
        indices,
        [
            (min_x, min_z, color[3]),
            (min_x, max_z, color[3]),
            (max_x, max_z, color[3]),
            (max_x, min_z, color[3]),
        ],
    );
    let base = vertices.len() - 4;
    for vertex in &mut vertices[base..] {
        vertex.color = color;
    }
}

fn push_gradient_quad(
    vertices: &mut Vec<ColoredVertex>,
    indices: &mut Vec<u32>,
    points: [(f32, f32, f32); 4],
) {
    let base = vertices.len() as u32;
    for (x, z, alpha) in points {
        vertices.push(ColoredVertex {
            world_pos: [x, z, SHADOW_OVERLAY_DEPTH],
            color: [0.0, 0.0, 0.0, alpha],
        });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn mesh_if_not_empty(vertices: Vec<ColoredVertex>, indices: Vec<u32>) -> Option<ColoredMeshInput> {
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    Some(ColoredMeshInput {
        pass: OverlayPass::AfterStatic,
        vertices,
        indices,
    })
}

fn color_with_alpha(color: RgbaColor, alpha: f32) -> [f32; 4] {
    [
        color.r.clamp(0.0, 1.0),
        color.g.clamp(0.0, 1.0),
        color.b.clamp(0.0, 1.0),
        alpha,
    ]
}

fn neighbor_casts_edge_shadow(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
    cell: Cell,
    dx: i32,
    dz: i32,
) -> bool {
    cell_casts_edge_shadow(thing_defs, world, Cell::new(cell.x + dx, cell.z + dz))
}

fn cell_casts_edge_shadow(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
    cell: Cell,
) -> bool {
    world.cell_in_bounds(cell)
        && world
            .things_at(cell)
            .iter()
            .filter_map(|thing_idx| world.things().get(*thing_idx))
            .any(|thing| thing_def(thing_defs, thing).is_some_and(|def| def.cast_edge_shadows))
}

fn thing_def<'a>(
    thing_defs: &'a HashMap<String, ThingDef>,
    thing: &ThingState,
) -> Option<&'a ThingDef> {
    thing_defs.get(&thing.def_name)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use glam::{Vec2, Vec3};

    use crate::defs::{GraphicData, GraphicKind, RgbaColor, ThingDef};
    use crate::fixtures::{
        FixtureColor, FixtureVector2, MapSpec, RenderSpec, SceneFixture, TerrainCell, ThingSpawn,
    };
    use crate::linking::{LinkDrawerType, LinkFlags};
    use crate::world::world_from_fixture;

    use super::{EDGE_SHADOW_IN_DIST, build_shadow_overlays};

    fn soil(count: usize) -> Vec<TerrainCell> {
        vec![
            TerrainCell {
                terrain_def: "Soil".to_string(),
            };
            count
        ]
    }

    fn thing_def(def_name: &str, cast_edge_shadows: bool, sun_height: f32) -> ThingDef {
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
            cast_edge_shadows,
            static_sun_shadow_height: sun_height,
            glower: None,
        }
    }

    #[test]
    fn no_shadow_defs_return_no_overlays() {
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
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        assert!(build_shadow_overlays(&HashMap::new(), &world).is_empty());
    }

    #[test]
    fn static_sun_shadow_uses_fixture_shadow_vector() {
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
                shadow_color: Some(FixtureColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                }),
                shadow_vector: Some(FixtureVector2 { x: 0.5, z: -0.25 }),
                ..RenderSpec::default()
            },
            things: vec![ThingSpawn {
                def_name: "ShadowCaster".to_string(),
                cell_x: 1,
                cell_z: 1,
                blocks_movement: true,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let thing_defs = HashMap::from([(
            "ShadowCaster".to_string(),
            thing_def("ShadowCaster", false, 0.4),
        )]);

        let overlays = build_shadow_overlays(&thing_defs, &world);

        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].vertices[0].world_pos[0], 1.2);
        assert_eq!(overlays[0].vertices[0].world_pos[1], 0.9);
        assert_eq!(overlays[0].vertices[0].color[3], 0.2);
    }

    #[test]
    fn edge_shadow_emits_interior_fade_band() {
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
                def_name: "EdgeCaster".to_string(),
                cell_x: 1,
                cell_z: 0,
                blocks_movement: true,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let thing_defs =
            HashMap::from([("EdgeCaster".to_string(), thing_def("EdgeCaster", true, 0.0))]);

        let overlays = build_shadow_overlays(&thing_defs, &world);

        assert_eq!(overlays.len(), 1);
        assert!(
            overlays[0]
                .vertices
                .iter()
                .any(|vertex| (vertex.world_pos[0] - (1.0 - EDGE_SHADOW_IN_DIST)).abs() < 0.001)
        );
    }
}
