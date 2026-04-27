use crate::renderer::{ColoredMeshInput, OverlayBlendMode, OverlayPass};
use crate::world::{RenderState, WorldState};

use super::solid_overlay_mesh::{SOLID_CELL_VERTEX_COUNT, mesh_if_not_empty, push_solid_cell};

const FOG_OVERLAY_DEPTH: f32 = -0.04;
const FOG_BASE_COLOR: [f32; 3] = [77.0 / 255.0, 69.0 / 255.0, 66.0 / 255.0];

pub fn build_fog_overlays(world: &WorldState) -> Vec<ColoredMeshInput> {
    let render = world.render_state();
    if !render.fog.iter().any(|fogged| *fogged) {
        return Vec::new();
    }

    let mut vertices = Vec::with_capacity(world.width() * world.height() * SOLID_CELL_VERTEX_COUNT);
    let mut indices = Vec::with_capacity(world.width() * world.height() * 24);
    let fog_color = fog_material_color(render);

    for z in 0..world.height() {
        for x in 0..world.width() {
            let covered = fog_covered_vertices(world, x, z);
            let colors = covered.map(|covered| fog_vertex_color(fog_color, covered));
            push_solid_cell(&mut vertices, &mut indices, x, z, FOG_OVERLAY_DEPTH, colors);
        }
    }

    mesh_if_not_empty(
        OverlayPass::AfterDynamic,
        OverlayBlendMode::Alpha,
        vertices,
        indices,
    )
}

fn fog_covered_vertices(world: &WorldState, x: usize, z: usize) -> [bool; SOLID_CELL_VERTEX_COUNT] {
    if fogged_at(world, x as i32, z as i32) {
        return [true; SOLID_CELL_VERTEX_COUNT];
    }

    let x = x as i32;
    let z = z as i32;
    let mut covered = [false; SOLID_CELL_VERTEX_COUNT];
    if fogged_at(world, x, z + 1) {
        covered[2] = true;
        covered[3] = true;
        covered[4] = true;
    }
    if fogged_at(world, x, z - 1) {
        covered[6] = true;
        covered[7] = true;
        covered[0] = true;
    }
    if fogged_at(world, x + 1, z) {
        covered[4] = true;
        covered[5] = true;
        covered[6] = true;
    }
    if fogged_at(world, x - 1, z) {
        covered[0] = true;
        covered[1] = true;
        covered[2] = true;
    }
    if fogged_at(world, x - 1, z - 1) {
        covered[0] = true;
    }
    if fogged_at(world, x - 1, z + 1) {
        covered[2] = true;
    }
    if fogged_at(world, x + 1, z + 1) {
        covered[4] = true;
    }
    if fogged_at(world, x + 1, z - 1) {
        covered[6] = true;
    }
    covered
}

fn fogged_at(world: &WorldState, x: i32, z: i32) -> bool {
    if x < 0 || z < 0 {
        return false;
    }
    let (x, z) = (x as usize, z as usize);
    if x >= world.width() || z >= world.height() {
        return false;
    }
    world.render_state().fog[z * world.width() + x]
}

fn fog_material_color(render: &RenderState) -> [f32; 3] {
    let Some(sky_glow) = render.sky_glow else {
        return FOG_BASE_COLOR;
    };
    let sky_color = color_rgb01(sky_glow);
    [
        FOG_BASE_COLOR[0] * sky_color[0],
        FOG_BASE_COLOR[1] * sky_color[1],
        FOG_BASE_COLOR[2] * sky_color[2],
    ]
}

fn fog_vertex_color(fog_color: [f32; 3], covered: bool) -> [f32; 4] {
    [
        fog_color[0],
        fog_color[1],
        fog_color[2],
        if covered { 1.0 } else { 0.0 },
    ]
}

fn color_rgb01(color: crate::defs::RgbaColor) -> [f32; 3] {
    let max_component = color.r.max(color.g).max(color.b);
    let scale = if max_component > 1.0 { 255.0 } else { 1.0 };
    [
        (color.r / scale).clamp(0.0, 1.0),
        (color.g / scale).clamp(0.0, 1.0),
        (color.b / scale).clamp(0.0, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use crate::fixtures::{FixtureColor, MapSpec, RenderSpec, SceneFixture, TerrainCell};
    use crate::renderer::OverlayPass;
    use crate::world::world_from_fixture;

    use super::build_fog_overlays;

    #[test]
    fn no_fog_returns_no_overlay() {
        let world = world_from_fixture(&fixture(vec![false; 4]));

        assert!(build_fog_overlays(&world).is_empty());
    }

    #[test]
    fn fogged_cell_sets_all_cell_vertices_opaque() {
        let world = world_from_fixture(&fixture(vec![
            false, false, false, false, true, false, false, false, false,
        ]));

        let overlays = build_fog_overlays(&world);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].pass, OverlayPass::AfterDynamic);
        assert_eq!(overlays[0].vertices.len(), 81);
        assert_eq!(overlays[0].indices.len(), 216);

        let center_start = 4 * 9;
        assert!(
            overlays[0].vertices[center_start..center_start + 9]
                .iter()
                .all(|vertex| vertex.color[3] == 1.0)
        );
    }

    #[test]
    fn unfogged_neighbor_gets_only_rimworld_boundary_vertices() {
        let world = world_from_fixture(&fixture(vec![
            false, false, false, false, true, false, false, false, false,
        ]));

        let overlay = build_fog_overlays(&world).pop().unwrap();
        let west_cell_start = 3 * 9;
        let alphas: Vec<f32> = overlay.vertices[west_cell_start..west_cell_start + 9]
            .iter()
            .map(|vertex| vertex.color[3])
            .collect();

        assert_eq!(alphas, vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn fog_color_multiplies_fixture_sky_color() {
        let mut fixture = fixture(vec![true]);
        fixture.render.sky_glow = Some(FixtureColor {
            r: 0.5,
            g: 0.25,
            b: 1.0,
            a: 1.0,
        });
        let world = world_from_fixture(&fixture);

        let overlay = build_fog_overlays(&world).pop().unwrap();
        let color = overlay.vertices[0].color;

        assert_color_close(
            color,
            [77.0 / 255.0 * 0.5, 69.0 / 255.0 * 0.25, 66.0 / 255.0, 1.0],
        );
    }

    #[test]
    fn fog_color_accepts_byte_sized_fixture_sky_color() {
        let mut fixture = fixture(vec![true]);
        fixture.render.sky_glow = Some(FixtureColor {
            r: 128.0,
            g: 64.0,
            b: 255.0,
            a: 1.0,
        });
        let world = world_from_fixture(&fixture);

        let overlay = build_fog_overlays(&world).pop().unwrap();
        let color = overlay.vertices[0].color;

        assert_color_close(
            color,
            [
                77.0 / 255.0 * (128.0 / 255.0),
                69.0 / 255.0 * (64.0 / 255.0),
                66.0 / 255.0,
                1.0,
            ],
        );
    }

    fn fixture(fog: Vec<bool>) -> SceneFixture {
        let cell_count = fog.len();
        let width = (cell_count as f32).sqrt() as usize;
        SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width,
                height: cell_count / width,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string()
                    };
                    cell_count
                ],
                roofs: Vec::new(),
                fog,
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        }
    }

    fn assert_color_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
        }
    }
}
