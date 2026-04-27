use std::collections::HashMap;

use glam::Vec2;

use crate::cell::Cell;
use crate::scene::{FULL_UV_RECT, MaterialKind, SpriteParams, SpriteRecord, TextureHandle};

use super::V2FrameOutput;

pub type PawnNodeTextureCache = HashMap<usize, HashMap<String, TextureHandle>>;

pub fn compose_dynamic_sprites(
    base_dynamic_inputs: &[SpriteRecord],
    pawn_node_textures: &PawnNodeTextureCache,
    overlay_texture_id: TextureHandle,
    frame: &V2FrameOutput,
) -> Vec<SpriteRecord> {
    let mut out = base_dynamic_inputs.to_vec();

    for node in &frame.pawn_nodes {
        let Some(pawn_nodes) = pawn_node_textures.get(&node.pawn_id) else {
            continue;
        };
        let Some(texture_id) = pawn_nodes.get(&node.node_id) else {
            continue;
        };

        out.push(SpriteRecord {
            texture: *texture_id,
            params: node.params.clone(),
            material: MaterialKind::Cutout,
        });
    }

    for cell in &frame.selected_path_cells {
        out.push(SpriteRecord {
            texture: overlay_texture_id,
            params: SpriteParams {
                world_pos: glam::Vec3::new(cell.x as f32 + 0.5, cell.z as f32 + 0.5, -0.23),
                size: Vec2::new(0.36, 0.36),
                tint: [0.35, 1.00, 0.45, 0.65],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    }

    if let Some(Cell { x, z }) = frame.hovered_cell {
        out.push(SpriteRecord {
            texture: overlay_texture_id,
            params: SpriteParams {
                world_pos: glam::Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.22),
                size: Vec2::new(1.04, 1.04),
                tint: [0.20, 0.90, 1.00, 0.28],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    }

    if let Some(selected_pos) = frame.selected_world_pos {
        out.push(SpriteRecord {
            texture: overlay_texture_id,
            params: SpriteParams {
                world_pos: glam::Vec3::new(selected_pos.x, selected_pos.y, -0.21),
                size: Vec2::new(1.16, 1.16),
                tint: [1.00, 0.90, 0.20, 0.30],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    } else if let Some(Cell { x, z }) = frame.selected_cell {
        out.push(SpriteRecord {
            texture: overlay_texture_id,
            params: SpriteParams {
                world_pos: glam::Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.21),
                size: Vec2::new(1.10, 1.10),
                tint: [1.00, 0.90, 0.20, 0.30],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    }

    out
}
