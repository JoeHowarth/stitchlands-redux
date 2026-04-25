use std::collections::HashMap;

use anyhow::Result;

use crate::cell::Cell;
use crate::defs::ThingDef;
use crate::renderer::{ColoredMeshInput, ColoredVertex, OverlayPass};
use crate::world::WorldState;

use super::glow_grid::GlowGrid;

const LIGHTING_OVERLAY_DEPTH: f32 = -0.20;
const ROOF_ALPHA_FLOOR: f32 = 100.0 / 255.0;
const MAX_DARKNESS_ALPHA: f32 = 0.72;

#[derive(Clone, Copy, Debug, Default)]
struct CellLighting {
    color: [f32; 4],
    block_light: bool,
    roof_forces_alpha: bool,
    roofed_without_support: bool,
}

pub fn build_lighting_overlays(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<Vec<ColoredMeshInput>> {
    if !has_lighting_inputs(world) && !GlowGrid::has_inputs(thing_defs, world) {
        return Ok(Vec::new());
    }

    let cell_lighting = build_cell_lighting(thing_defs, world);
    let width = world.width();
    let height = world.height();
    let corner_count = (width + 1) * (height + 1);
    let mut vertices = Vec::with_capacity(corner_count + width * height);

    for z in 0..=height {
        for x in 0..=width {
            vertices.push(ColoredVertex {
                world_pos: [x as f32, z as f32, LIGHTING_OVERLAY_DEPTH],
                color: corner_color(&cell_lighting, width, height, x, z),
            });
        }
    }

    for z in 0..height {
        for x in 0..width {
            let bot_left = z * (width + 1) + x;
            let mut color = average_colors([
                vertices[bot_left].color,
                vertices[bot_left + 1].color,
                vertices[bot_left + width + 1].color,
                vertices[bot_left + width + 2].color,
            ]);
            let cell = cell_lighting[z * width + x];
            if cell.roofed_without_support && color[3] < ROOF_ALPHA_FLOOR {
                color[3] = ROOF_ALPHA_FLOOR;
            }
            vertices.push(ColoredVertex {
                world_pos: [x as f32 + 0.5, z as f32 + 0.5, LIGHTING_OVERLAY_DEPTH],
                color,
            });
        }
    }

    if vertices.iter().all(|vertex| vertex.color[3] <= 0.0) {
        return Ok(Vec::new());
    }

    let first_center = corner_count as u32;
    let mut indices = Vec::with_capacity(width * height * 12);
    for z in 0..height {
        for x in 0..width {
            let bot_left = (z * (width + 1) + x) as u32;
            let top_left = ((z + 1) * (width + 1) + x) as u32;
            let top_right = top_left + 1;
            let bot_right = bot_left + 1;
            let center = first_center + (z * width + x) as u32;
            indices.extend_from_slice(&[
                bot_left, center, bot_right, bot_left, top_left, center, top_left, top_right,
                center, top_right, bot_right, center,
            ]);
        }
    }

    Ok(vec![ColoredMeshInput {
        pass: OverlayPass::AfterStatic,
        vertices,
        indices,
    }])
}

fn has_lighting_inputs(world: &WorldState) -> bool {
    let render = world.render_state();
    render.day_percent.is_some()
        || render.sky_glow.is_some()
        || render.roofs.iter().any(|roof| roof.roofed)
}

fn build_cell_lighting(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Vec<CellLighting> {
    let render = world.render_state();
    let sky_brightness = sky_brightness(world);
    let glow_grid = GlowGrid::from_world(thing_defs, world);
    let mut cells = Vec::with_capacity(world.width() * world.height());

    for z in 0..world.height() {
        for x in 0..world.width() {
            let cell = Cell::new(x as i32, z as i32);
            let index = z * world.width() + x;
            let block_light = cell_has_block_light(thing_defs, world, cell);
            let holds_roof = cell_holds_roof(thing_defs, world, cell);
            let roof = render.roofs[index];
            let brightness = (sky_brightness + glow_grid.visual_glow_at(cell)).min(1.0);
            let mut alpha = ((1.0 - brightness) * MAX_DARKNESS_ALPHA).clamp(0.0, 1.0);
            let roof_forces_alpha = roof.roofed && (roof.thick || !holds_roof);
            if roof_forces_alpha && alpha < ROOF_ALPHA_FLOOR {
                alpha = ROOF_ALPHA_FLOOR;
            }
            cells.push(CellLighting {
                color: [0.0, 0.0, 0.0, alpha],
                block_light,
                roof_forces_alpha,
                roofed_without_support: roof.roofed && !holds_roof,
            });
        }
    }

    cells
}

fn sky_brightness(world: &WorldState) -> f32 {
    let render = world.render_state();
    if let Some(color) = render.sky_glow {
        return ((color.r + color.g + color.b) / 3.0).clamp(0.0, 1.0);
    }
    if let Some(day_percent) = render.day_percent {
        let daylight = 1.0 - (day_percent - 0.5).abs() * 2.0;
        return (0.12 + daylight.max(0.0) * 0.88).clamp(0.0, 1.0);
    }
    1.0
}

fn corner_color(
    cell_lighting: &[CellLighting],
    width: usize,
    height: usize,
    x: usize,
    z: usize,
) -> [f32; 4] {
    let mut colors = Vec::with_capacity(4);
    let mut roof_forces_alpha = false;
    for (sample_x, sample_z) in [
        (x.checked_sub(1), z.checked_sub(1)),
        (x.checked_sub(1), Some(z)),
        (Some(x), z.checked_sub(1)),
        (Some(x), Some(z)),
    ] {
        let (Some(sample_x), Some(sample_z)) = (sample_x, sample_z) else {
            continue;
        };
        if sample_x >= width || sample_z >= height {
            continue;
        }
        let sample = cell_lighting[sample_z * width + sample_x];
        roof_forces_alpha |= sample.roof_forces_alpha;
        if !sample.block_light {
            colors.push(sample.color);
        }
    }

    let mut color = average_color_slice(&colors);
    if roof_forces_alpha && color[3] < ROOF_ALPHA_FLOOR {
        color[3] = ROOF_ALPHA_FLOOR;
    }
    color
}

fn average_colors(colors: [[f32; 4]; 4]) -> [f32; 4] {
    let mut out = [0.0; 4];
    for color in colors {
        for i in 0..4 {
            out[i] += color[i];
        }
    }
    for value in &mut out {
        *value /= 4.0;
    }
    out
}

fn average_color_slice(colors: &[[f32; 4]]) -> [f32; 4] {
    if colors.is_empty() {
        return [0.0; 4];
    }
    let mut out = [0.0; 4];
    for color in colors {
        for i in 0..4 {
            out[i] += color[i];
        }
    }
    for value in &mut out {
        *value /= colors.len() as f32;
    }
    out
}

fn cell_has_block_light(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
    cell: Cell,
) -> bool {
    world.things_at(cell).iter().any(|thing_idx| {
        world
            .things()
            .get(*thing_idx)
            .and_then(|thing| thing_defs.get(&thing.def_name))
            .map(|def| def.block_light)
            .unwrap_or(false)
    })
}

fn cell_holds_roof(thing_defs: &HashMap<String, ThingDef>, world: &WorldState, cell: Cell) -> bool {
    world.things_at(cell).iter().any(|thing_idx| {
        world
            .things()
            .get(*thing_idx)
            .and_then(|thing| thing_defs.get(&thing.def_name))
            .map(|def| def.holds_roof)
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use glam::{Vec2, Vec3};

    use crate::defs::{GlowerProps, GraphicData, GraphicKind, RgbaColor, ThingDef};
    use crate::fixtures::{MapSpec, RenderSpec, RoofCell, SceneFixture, TerrainCell, ThingSpawn};
    use crate::linking::{LinkDrawerType, LinkFlags};
    use crate::world::world_from_fixture;

    use super::{ROOF_ALPHA_FLOOR, build_lighting_overlays};

    fn soil(count: usize) -> Vec<TerrainCell> {
        vec![
            TerrainCell {
                terrain_def: "Soil".to_string(),
            };
            count
        ]
    }

    fn thing_def(def_name: &str, block_light: bool, holds_roof: bool) -> ThingDef {
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
            holds_roof,
            cast_edge_shadows: false,
            static_sun_shadow_height: 0.0,
            glower: None,
        }
    }

    #[test]
    fn no_lighting_inputs_returns_no_overlay() {
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
            build_lighting_overlays(&HashMap::new(), &world)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn roofed_cells_force_minimum_overlay_alpha() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 1,
                height: 1,
                terrain: soil(1),
                roofs: vec![RoofCell {
                    roofed: true,
                    thick: true,
                }],
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        });

        let overlays = build_lighting_overlays(&HashMap::new(), &world).unwrap();

        assert_eq!(overlays.len(), 1);
        assert!(
            overlays[0]
                .vertices
                .iter()
                .all(|vertex| vertex.color[3] >= ROOF_ALPHA_FLOOR)
        );
    }

    #[test]
    fn block_light_cells_are_skipped_for_corner_averages() {
        let world = world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 2,
                height: 1,
                terrain: soil(2),
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec {
                day_percent: Some(0.0),
                shadow_vector: None,
                ..RenderSpec::default()
            },
            things: vec![ThingSpawn {
                def_name: "Blocker".to_string(),
                cell_x: 0,
                cell_z: 0,
                blocks_movement: true,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let thing_defs =
            HashMap::from([("Blocker".to_string(), thing_def("Blocker", true, false))]);

        let overlays = build_lighting_overlays(&thing_defs, &world).unwrap();

        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].vertices[0].color[3], 0.0);
        assert!(overlays[0].vertices[1].color[3] > 0.0);
    }

    #[test]
    fn glower_thing_brightens_nearby_cells() {
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
                day_percent: Some(0.0),
                ..RenderSpec::default()
            },
            things: vec![ThingSpawn {
                def_name: "Lamp".to_string(),
                cell_x: 0,
                cell_z: 0,
                blocks_movement: false,
            }],
            pawns: Vec::new(),
            camera: None,
        });
        let mut lamp = thing_def("Lamp", false, false);
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

        let overlays = build_lighting_overlays(&thing_defs, &world).unwrap();

        assert_eq!(overlays.len(), 1);
        let first_center = (world.width() + 1) * (world.height() + 1);
        let near_alpha = overlays[0].vertices[first_center].color[3];
        let far_alpha = overlays[0].vertices[first_center + 2].color[3];
        assert!(near_alpha < far_alpha);
    }
}
