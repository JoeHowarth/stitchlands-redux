use image::RgbaImage;

use crate::scene::{Layer, MaterialKind, TexturedMeshInput, TexturedVertex};

pub const SOLID_CELL_VERTEX_COUNT: usize = 9;

const SOLID_CELL_VERTEX_OFFSETS: [(f32, f32); SOLID_CELL_VERTEX_COUNT] = [
    (0.0, 0.0),
    (0.0, 0.5),
    (0.0, 1.0),
    (0.5, 1.0),
    (1.0, 1.0),
    (1.0, 0.5),
    (1.0, 0.0),
    (0.5, 0.0),
    (0.5, 0.5),
];

const SOLID_CELL_TRI_INDICES: [u32; 24] = [
    7, 0, 1, 1, 2, 3, 3, 4, 5, 5, 6, 7, 7, 1, 8, 1, 3, 8, 3, 5, 8, 5, 7, 8,
];

pub fn push_textured_solid_cell(
    vertices: &mut Vec<TexturedVertex>,
    indices: &mut Vec<u32>,
    x: usize,
    z: usize,
    depth: f32,
    colors: [[f32; 4]; SOLID_CELL_VERTEX_COUNT],
) {
    let base = vertices.len() as u32;
    for (idx, (offset_x, offset_z)) in SOLID_CELL_VERTEX_OFFSETS.iter().enumerate() {
        let world_x = x as f32 + offset_x;
        let world_z = z as f32 + offset_z;
        vertices.push(TexturedVertex {
            world_pos: [world_x, world_z, depth],
            uv: [world_x, world_z],
            color: colors[idx],
        });
    }
    indices.extend(SOLID_CELL_TRI_INDICES.iter().map(|index| base + index));
}

pub fn textured_mesh_if_not_empty(
    layer: Layer,
    material: MaterialKind,
    image: RgbaImage,
    vertices: Vec<TexturedVertex>,
    indices: Vec<u32>,
) -> Vec<TexturedMeshInput> {
    if vertices.is_empty() || indices.is_empty() {
        return Vec::new();
    }
    vec![TexturedMeshInput {
        layer,
        material,
        image,
        vertices,
        indices,
    }]
}
