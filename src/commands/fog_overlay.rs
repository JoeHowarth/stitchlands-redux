use anyhow::{Context, Result};
use image::RgbaImage;

use crate::assets::AssetResolver;
use crate::renderer::{OverlayPass, TexturedMeshInput};
use crate::world::{RenderState, WorldState};

use super::solid_overlay_mesh::{
    SOLID_CELL_VERTEX_COUNT, push_textured_solid_cell, textured_mesh_if_not_empty,
};

const FOG_OVERLAY_DEPTH: f32 = -0.04;
const FOG_BASE_COLOR: [f32; 3] = [77.0 / 255.0, 69.0 / 255.0, 66.0 / 255.0];
const FOG_MATERIAL_TEXTURE_PATH: &str = "Misc/FogOfWar";
const FOG_OPACITY_SCALE: f32 = 0.85;

pub fn build_fog_overlays(
    asset_resolver: &mut AssetResolver,
    world: &WorldState,
) -> Result<Vec<TexturedMeshInput>> {
    let render = world.render_state();
    if !render.fog.iter().any(|fogged| *fogged) {
        return Ok(Vec::new());
    }

    let fog_material = load_fog_material_texture(asset_resolver)?;
    let mut vertices = Vec::with_capacity(world.width() * world.height() * SOLID_CELL_VERTEX_COUNT);
    let mut indices = Vec::with_capacity(world.width() * world.height() * 24);
    let fog_color = fog_material_color(render);

    for z in 0..world.height() {
        for x in 0..world.width() {
            let covered = fog_covered_vertices(world, x, z);
            let colors = covered.map(|covered| fog_vertex_color(fog_color, covered));
            push_textured_solid_cell(&mut vertices, &mut indices, x, z, FOG_OVERLAY_DEPTH, colors);
        }
    }

    Ok(textured_mesh_if_not_empty(
        OverlayPass::AfterDynamic,
        fog_material,
        vertices,
        indices,
    ))
}

fn load_fog_material_texture(asset_resolver: &mut AssetResolver) -> Result<RgbaImage> {
    let resolved = asset_resolver
        .resolve_texture_path(FOG_MATERIAL_TEXTURE_PATH)
        .with_context(|| format!("resolving fog material texture '{FOG_MATERIAL_TEXTURE_PATH}'"))?;
    if resolved.used_fallback() {
        anyhow::bail!("missing fog material texture '{FOG_MATERIAL_TEXTURE_PATH}'");
    }

    Ok(fog_texture_with_luminance_alpha(resolved.image))
}

fn fog_texture_with_luminance_alpha(mut image: RgbaImage) -> RgbaImage {
    for pixel in image.pixels_mut() {
        let [r, g, b, _] = pixel.0;
        let luminance = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32).round();
        pixel.0[3] = luminance.clamp(0.0, 255.0) as u8;
    }
    image
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
        if covered { FOG_OPACITY_SCALE } else { 0.0 },
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
    use image::{Rgba, RgbaImage};

    use crate::fixtures::{FixtureColor, MapSpec, RenderSpec, SceneFixture, TerrainCell};
    use crate::world::world_from_fixture;

    use super::{
        FOG_MATERIAL_TEXTURE_PATH, FOG_OPACITY_SCALE, fog_texture_with_luminance_alpha,
        fog_vertex_color,
    };

    #[test]
    fn no_fog_returns_no_overlay() {
        let world = world_from_fixture(&fixture(vec![false; 4]));

        assert!(!world.render_state().fog.iter().any(|fogged| *fogged));
    }

    #[test]
    fn fogged_cell_sets_all_cell_vertices_to_scaled_opacity() {
        let world = world_from_fixture(&fixture(vec![
            false, false, false, false, true, false, false, false, false,
        ]));

        let center = super::fog_covered_vertices(&world, 1, 1);
        assert!(
            center
                .iter()
                .map(|covered| fog_vertex_color([1.0, 1.0, 1.0], *covered))
                .all(|color| color[3] == FOG_OPACITY_SCALE)
        );
    }

    #[test]
    fn unfogged_neighbor_gets_only_rimworld_boundary_vertices() {
        let world = world_from_fixture(&fixture(vec![
            false, false, false, false, true, false, false, false, false,
        ]));

        let alphas: Vec<f32> = super::fog_covered_vertices(&world, 0, 1)
            .iter()
            .map(|covered| fog_vertex_color([1.0, 1.0, 1.0], *covered)[3])
            .collect();

        assert_eq!(
            alphas,
            vec![
                0.0,
                0.0,
                0.0,
                0.0,
                FOG_OPACITY_SCALE,
                FOG_OPACITY_SCALE,
                FOG_OPACITY_SCALE,
                0.0,
                0.0
            ]
        );
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

        let color = fog_vertex_color(super::fog_material_color(world.render_state()), true);

        assert_color_close(
            color,
            [
                77.0 / 255.0 * 0.5,
                69.0 / 255.0 * 0.25,
                66.0 / 255.0,
                FOG_OPACITY_SCALE,
            ],
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

        let color = fog_vertex_color(super::fog_material_color(world.render_state()), true);

        assert_color_close(
            color,
            [
                77.0 / 255.0 * (128.0 / 255.0),
                69.0 / 255.0 * (64.0 / 255.0),
                66.0 / 255.0,
                FOG_OPACITY_SCALE,
            ],
        );
    }

    #[test]
    fn fog_material_texture_path_matches_rimworld_material_resource() {
        assert_eq!(FOG_MATERIAL_TEXTURE_PATH, "Misc/FogOfWar");
    }

    #[test]
    fn fog_texture_luminance_drives_alpha_variation() {
        let image =
            RgbaImage::from_vec(2, 1, vec![200, 200, 200, 255, 240, 240, 240, 255]).unwrap();

        let image = fog_texture_with_luminance_alpha(image);

        assert_eq!(image.get_pixel(0, 0), &Rgba([200, 200, 200, 200]));
        assert_eq!(image.get_pixel(1, 0), &Rgba([240, 240, 240, 240]));
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
