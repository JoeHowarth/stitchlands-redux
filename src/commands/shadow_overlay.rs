use std::collections::HashMap;

use anyhow::Result;
use glam::Vec2;

use crate::cell::Cell;
use crate::defs::{RgbaColor, ShadowData, ThingDef};
use crate::renderer::{ColoredMeshInput, ColoredVertex, OverlayPass};
use crate::world::{ThingState, WorldState};

use super::sky_shadow::sky_shadow_state;

const SHADOW_OVERLAY_DEPTH: f32 = -0.18;
const EDGE_SHADOW_IN_DIST: f32 = 0.45;
const EDGE_SHADOW_ALPHA: f32 = (255.0 - 195.0) / 255.0;
const EDGE_SHADOW_COLOR: RgbaColor = RgbaColor {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub fn build_shadow_overlays(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<Vec<ColoredMeshInput>> {
    let mut overlays = Vec::new();
    if let Some(sun) = build_sun_shadow_overlay(thing_defs, world)? {
        overlays.push(sun);
    }
    if let Some(edge) = build_edge_shadow_overlay(thing_defs, world) {
        overlays.push(edge);
    }
    if let Some(graphic) = build_graphic_shadow_overlay(thing_defs, world)? {
        overlays.push(graphic);
    }
    Ok(overlays)
}

fn build_sun_shadow_overlay(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<Option<ColoredMeshInput>> {
    if !world.things().iter().any(|thing| {
        thing_defs
            .get(&thing.def_name)
            .is_some_and(|def| def.static_sun_shadow_height > 0.0)
    }) {
        return Ok(None);
    }

    let sky_shadow = sky_shadow_state(world.render_state())?;
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
        let offset = sky_shadow.shadow_vector * height.max(0.0);
        let alpha = (sky_shadow.shadow_alpha_scale * height).clamp(0.0, 1.0);
        push_solid_quad(
            &mut vertices,
            &mut indices,
            thing.cell_x as f32 + offset.x,
            thing.cell_z as f32 + offset.y,
            thing.cell_x as f32 + 1.0 + offset.x,
            thing.cell_z as f32 + 1.0 + offset.y,
            color_with_alpha(sky_shadow.shadow_color, alpha),
        );
    }

    Ok(mesh_if_not_empty(vertices, indices))
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
                    EDGE_SHADOW_COLOR,
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
                    EDGE_SHADOW_COLOR,
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
                    EDGE_SHADOW_COLOR,
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
                    EDGE_SHADOW_COLOR,
                );
            }
        }
    }

    mesh_if_not_empty(vertices, indices)
}

fn build_graphic_shadow_overlay(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<Option<ColoredMeshInput>> {
    if !world.things().iter().any(|thing| {
        thing_def(thing_defs, thing).is_some_and(|def| def.graphic_data.shadow_data.is_some())
    }) {
        return Ok(None);
    }

    let sky_shadow = sky_shadow_state(world.render_state())?;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for thing in world.things() {
        let Some(shadow) =
            thing_def(thing_defs, thing).and_then(|def| def.graphic_data.shadow_data)
        else {
            continue;
        };
        push_graphic_shadow(
            &mut vertices,
            &mut indices,
            thing,
            shadow,
            sky_shadow.shadow_vector,
            sky_shadow.shadow_alpha_scale,
            sky_shadow.shadow_color,
        );
    }

    Ok(mesh_if_not_empty(vertices, indices))
}

fn push_graphic_shadow(
    vertices: &mut Vec<ColoredVertex>,
    indices: &mut Vec<u32>,
    thing: &ThingState,
    shadow: ShadowData,
    shadow_vector: Vec2,
    shadow_alpha_scale: f32,
    shadow_color: RgbaColor,
) {
    let half_width = shadow.volume.x.abs() * 0.5;
    let length = shadow.volume.z.abs();
    let alpha = (shadow.volume.y * shadow_alpha_scale).clamp(0.0, 1.0);
    if half_width <= 0.0 || length <= 0.0 || alpha <= 0.0 {
        return;
    }

    let direction = shadow_vector.try_normalize().unwrap_or(Vec2::Y);
    let perp = Vec2::new(-direction.y, direction.x);
    let center = Vec2::new(
        thing.cell_x as f32 + 0.5 + shadow.offset.x,
        thing.cell_z as f32 + 0.5 + shadow.offset.z,
    );
    let near_center = center - direction * (length * 0.15);
    let far_center = center + direction * (length * 0.85);
    let core_half_width = half_width * 0.55;
    let near_outer_left = near_center - perp * half_width;
    let near_inner_left = near_center - perp * core_half_width;
    let near_inner_right = near_center + perp * core_half_width;
    let near_outer_right = near_center + perp * half_width;
    let far_outer_left = far_center - perp * half_width;
    let far_inner_left = far_center - perp * core_half_width;
    let far_inner_right = far_center + perp * core_half_width;
    let far_outer_right = far_center + perp * half_width;

    push_gradient_quad(
        vertices,
        indices,
        [
            (near_outer_left.x, near_outer_left.y, 0.0),
            (near_inner_left.x, near_inner_left.y, 0.0),
            (far_inner_left.x, far_inner_left.y, alpha),
            (far_outer_left.x, far_outer_left.y, 0.0),
        ],
        shadow_color,
    );
    push_gradient_quad(
        vertices,
        indices,
        [
            (near_inner_left.x, near_inner_left.y, 0.0),
            (near_inner_right.x, near_inner_right.y, 0.0),
            (far_inner_right.x, far_inner_right.y, alpha),
            (far_inner_left.x, far_inner_left.y, alpha),
        ],
        shadow_color,
    );
    push_gradient_quad(
        vertices,
        indices,
        [
            (near_inner_right.x, near_inner_right.y, 0.0),
            (near_outer_right.x, near_outer_right.y, 0.0),
            (far_outer_right.x, far_outer_right.y, 0.0),
            (far_inner_right.x, far_inner_right.y, alpha),
        ],
        shadow_color,
    );
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
        RgbaColor {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        },
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
    color: RgbaColor,
) {
    let base = vertices.len() as u32;
    for (x, z, alpha) in points {
        vertices.push(ColoredVertex {
            world_pos: [x, z, SHADOW_OVERLAY_DEPTH],
            color: [
                color.r.clamp(0.0, 1.0),
                color.g.clamp(0.0, 1.0),
                color.b.clamp(0.0, 1.0),
                alpha,
            ],
        });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn mesh_if_not_empty(vertices: Vec<ColoredVertex>, indices: Vec<u32>) -> Option<ColoredMeshInput> {
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    Some(ColoredMeshInput {
        pass: OverlayPass::AfterTerrain,
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

    use crate::defs::{GraphicData, GraphicKind, RgbaColor, ShadowData, ThingDef};
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

    fn shadow_caster_world(day_percent: Option<f32>) -> crate::world::WorldState {
        world_from_fixture(&SceneFixture {
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
                day_percent,
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
        })
    }

    fn thing_def(
        def_name: &str,
        cast_edge_shadows: bool,
        sun_height: f32,
        shadow_data: Option<ShadowData>,
    ) -> ThingDef {
        ThingDef {
            def_name: def_name.to_string(),
            graphic_data: GraphicData {
                tex_path: "Things/Test".to_string(),
                kind: GraphicKind::Single,
                color: RgbaColor::WHITE,
                draw_size: Vec2::ONE,
                draw_offset: Vec3::ZERO,
                shadow_data,
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

        assert!(
            build_shadow_overlays(&HashMap::new(), &world)
                .unwrap()
                .is_empty()
        );
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
            thing_def("ShadowCaster", false, 0.4, None),
        )]);

        let overlays = build_shadow_overlays(&thing_defs, &world).unwrap();

        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].vertices[0].world_pos[0], 1.2);
        assert_eq!(overlays[0].vertices[0].world_pos[1], 0.9);
        assert_eq!(overlays[0].vertices[0].color[3], 0.2);
    }

    #[test]
    fn static_sun_shadow_derives_vector_from_day_percent() {
        let thing_defs = HashMap::from([(
            "ShadowCaster".to_string(),
            thing_def("ShadowCaster", false, 1.0, None),
        )]);
        let morning = shadow_caster_world(Some(0.35));
        let evening = shadow_caster_world(Some(0.65));

        let morning_overlays = build_shadow_overlays(&thing_defs, &morning).unwrap();
        let evening_overlays = build_shadow_overlays(&thing_defs, &evening).unwrap();

        assert!((morning_overlays[0].vertices[0].world_pos[0] + 3.5).abs() < 0.001);
        assert!((evening_overlays[0].vertices[0].world_pos[0] - 5.5).abs() < 0.001);
        assert_ne!(
            morning_overlays[0].vertices[0].world_pos[0],
            evening_overlays[0].vertices[0].world_pos[0]
        );
    }

    #[test]
    fn sun_shadow_without_sky_state_errors() {
        let world = shadow_caster_world(None);
        let thing_defs = HashMap::from([(
            "ShadowCaster".to_string(),
            thing_def("ShadowCaster", false, 1.0, None),
        )]);

        let err = build_shadow_overlays(&thing_defs, &world)
            .expect_err("sun shadow should require explicit or derived sky state")
            .to_string();

        assert!(err.contains("shadow overlays require render.day_percent"));
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
        let thing_defs = HashMap::from([(
            "EdgeCaster".to_string(),
            thing_def("EdgeCaster", true, 0.0, None),
        )]);

        let overlays = build_shadow_overlays(&thing_defs, &world).unwrap();

        assert_eq!(overlays.len(), 1);
        assert!(
            overlays[0]
                .vertices
                .iter()
                .any(|vertex| (vertex.world_pos[0] - (1.0 - EDGE_SHADOW_IN_DIST)).abs() < 0.001)
        );
    }

    #[test]
    fn graphic_shadow_uses_shadow_data_volume_and_offset() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 3,
                height: 3,
                terrain: soil(9),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                shadow_color: Some(FixtureColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                shadow_vector: Some(FixtureVector2 { x: 1.0, z: 0.0 }),
                ..RenderSpec::default()
            },
            things: vec![ThingSpawn {
                def_name: "PlantLike".to_string(),
                cell_x: 1,
                cell_z: 1,
                blocks_movement: false,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let thing_defs = HashMap::from([(
            "PlantLike".to_string(),
            thing_def(
                "PlantLike",
                false,
                0.0,
                Some(ShadowData {
                    volume: Vec3::new(1.2, 0.35, 0.8),
                    offset: Vec3::new(0.2, 0.0, -0.1),
                }),
            ),
        )]);

        let overlays = build_shadow_overlays(&thing_defs, &world).unwrap();

        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].vertices.len(), 12);
        assert_eq!(overlays[0].indices.len(), 18);
        assert!(
            overlays[0]
                .vertices
                .iter()
                .any(|vertex| (vertex.world_pos[0] - 1.58).abs() < 0.001
                    && (vertex.world_pos[1] - 0.8).abs() < 0.001
                    && vertex.color[3] == 0.0)
        );
        assert!(
            overlays[0]
                .vertices
                .iter()
                .any(|vertex| (vertex.world_pos[0] - 2.38).abs() < 0.001
                    && (vertex.world_pos[1] - 1.73).abs() < 0.001
                    && (vertex.color[3] - 0.35).abs() < 0.001)
        );
    }
}
