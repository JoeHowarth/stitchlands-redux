use crate::renderer::{ColoredMeshInput, OverlayBlendMode, OverlayPass};
use crate::world::WorldState;

use super::solid_overlay_mesh::{SOLID_CELL_VERTEX_COUNT, mesh_if_not_empty, push_solid_cell};

const SNOW_OVERLAY_DEPTH: f32 = -0.16;
const SNOW_VISIBLE_EPSILON: f32 = 0.01;
const SNOW_SAMPLE_OFFSETS: [(i32, i32); SOLID_CELL_VERTEX_COUNT] = [
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, 0),
];
const SNOW_VERTEX_WEIGHTS: [&[usize]; SOLID_CELL_VERTEX_COUNT] = [
    &[0, 1, 2, 8],
    &[2, 8],
    &[2, 3, 4, 8],
    &[4, 8],
    &[4, 5, 6, 8],
    &[6, 8],
    &[6, 7, 0, 8],
    &[0, 8],
    &[8],
];

pub fn build_snow_overlays(world: &WorldState) -> Vec<ColoredMeshInput> {
    let render = world.render_state();
    if !render
        .snow_depth
        .iter()
        .any(|depth| *depth > SNOW_VISIBLE_EPSILON)
    {
        return Vec::new();
    }

    let mut any_visible = false;
    let mut vertices = Vec::with_capacity(world.width() * world.height() * SOLID_CELL_VERTEX_COUNT);
    let mut indices = Vec::with_capacity(world.width() * world.height() * 24);

    for z in 0..world.height() {
        for x in 0..world.width() {
            let opacities = snow_vertex_opacities(world, x, z);
            any_visible |= opacities.iter().any(|alpha| *alpha > SNOW_VISIBLE_EPSILON);
            let colors = opacities.map(snow_vertex_color);
            push_solid_cell(
                &mut vertices,
                &mut indices,
                x,
                z,
                SNOW_OVERLAY_DEPTH,
                colors,
            );
        }
    }

    if !any_visible {
        return Vec::new();
    }

    mesh_if_not_empty(
        OverlayPass::AfterTerrain,
        OverlayBlendMode::Alpha,
        vertices,
        indices,
    )
}

fn snow_vertex_opacities(world: &WorldState, x: usize, z: usize) -> [f32; SOLID_CELL_VERTEX_COUNT] {
    let current_depth = snow_depth_at(world, x as i32, z as i32).unwrap_or(0.0);
    let mut adjacent_depths = [0.0; SOLID_CELL_VERTEX_COUNT];
    for (idx, (offset_x, offset_z)) in SNOW_SAMPLE_OFFSETS.iter().enumerate() {
        adjacent_depths[idx] =
            snow_depth_at(world, x as i32 + offset_x, z as i32 + offset_z).unwrap_or(current_depth);
    }

    let mut opacities = [0.0; SOLID_CELL_VERTEX_COUNT];
    for (idx, weights) in SNOW_VERTEX_WEIGHTS.iter().enumerate() {
        let total: f32 = weights
            .iter()
            .map(|weight_idx| adjacent_depths[*weight_idx])
            .sum();
        opacities[idx] = (total / weights.len() as f32).clamp(0.0, 1.0);
    }
    opacities
}

fn snow_depth_at(world: &WorldState, x: i32, z: i32) -> Option<f32> {
    if x < 0 || z < 0 {
        return None;
    }
    let (x, z) = (x as usize, z as usize);
    if x >= world.width() || z >= world.height() {
        return None;
    }
    Some(world.render_state().snow_depth[z * world.width() + x].clamp(0.0, 1.0))
}

fn snow_vertex_color(opacity: f32) -> [f32; 4] {
    [1.0, 1.0, 1.0, opacity]
}

#[cfg(test)]
mod tests {
    use crate::fixtures::{MapSpec, RenderSpec, SceneFixture, TerrainCell};
    use crate::renderer::OverlayPass;
    use crate::world::world_from_fixture;

    use super::build_snow_overlays;

    #[test]
    fn no_snow_returns_no_overlay() {
        let world = world_from_fixture(&fixture(2, 2, vec![0.0; 4]));

        assert!(build_snow_overlays(&world).is_empty());
    }

    #[test]
    fn center_snow_depth_uses_rimworld_vertex_weights() {
        let world = world_from_fixture(&fixture(
            3,
            3,
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        ));

        let overlays = build_snow_overlays(&world);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].pass, OverlayPass::AfterTerrain);
        assert_eq!(overlays[0].vertices.len(), 81);
        assert_eq!(overlays[0].indices.len(), 216);

        let center_start = 4 * 9;
        let alphas: Vec<f32> = overlays[0].vertices[center_start..center_start + 9]
            .iter()
            .map(|vertex| vertex.color[3])
            .collect();

        assert_eq!(
            alphas,
            vec![0.25, 0.5, 0.25, 0.5, 0.25, 0.5, 0.25, 0.5, 1.0]
        );
    }

    #[test]
    fn out_of_bounds_snow_samples_use_current_cell_depth() {
        let world = world_from_fixture(&fixture(1, 1, vec![0.5]));

        let overlay = build_snow_overlays(&world).pop().unwrap();
        assert!(
            overlay.vertices[0..9]
                .iter()
                .all(|vertex| vertex.color[3] == 0.5)
        );
    }

    fn fixture(width: usize, height: usize, snow_depth: Vec<f32>) -> SceneFixture {
        SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width,
                height,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string()
                    };
                    width * height
                ],
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth,
            },
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        }
    }
}
