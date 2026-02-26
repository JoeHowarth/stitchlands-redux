use glam::Vec2;
use image::RgbaImage;

use crate::renderer::{SpriteInput, SpriteParams};

use super::V2FrameOutput;

pub fn compose_dynamic_sprites(
    base_dynamic_inputs: &[SpriteInput],
    base_dynamic_pawn_ids: &[Option<usize>],
    overlay_image: &RgbaImage,
    frame: &V2FrameOutput,
) -> Vec<SpriteInput> {
    let mut out = base_dynamic_inputs.to_vec();

    for (sprite, pawn_id) in out.iter_mut().zip(base_dynamic_pawn_ids.iter()) {
        let Some(pawn_id) = pawn_id else {
            continue;
        };
        let Some(delta) = frame.pawn_offsets.get(pawn_id) else {
            continue;
        };
        sprite.params.world_pos.x += delta.x;
        sprite.params.world_pos.y += delta.y;
    }

    for cell in &frame.selected_path_cells {
        out.push(SpriteInput {
            image: overlay_image.clone(),
            params: SpriteParams {
                world_pos: glam::Vec3::new(cell.0 as f32 + 0.5, cell.1 as f32 + 0.5, -0.23),
                size: Vec2::new(0.36, 0.36),
                tint: [0.35, 1.00, 0.45, 0.65],
            },
        });
    }

    if let Some((x, z)) = frame.hovered_cell {
        out.push(SpriteInput {
            image: overlay_image.clone(),
            params: SpriteParams {
                world_pos: glam::Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.22),
                size: Vec2::new(1.04, 1.04),
                tint: [0.20, 0.90, 1.00, 0.28],
            },
        });
    }

    if let Some(selected_pos) = frame.selected_world_pos {
        out.push(SpriteInput {
            image: overlay_image.clone(),
            params: SpriteParams {
                world_pos: glam::Vec3::new(selected_pos.x, selected_pos.y, -0.21),
                size: Vec2::new(1.16, 1.16),
                tint: [1.00, 0.90, 0.20, 0.30],
            },
        });
    } else if let Some((x, z)) = frame.selected_cell {
        out.push(SpriteInput {
            image: overlay_image.clone(),
            params: SpriteParams {
                world_pos: glam::Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.21),
                size: Vec2::new(1.10, 1.10),
                tint: [1.00, 0.90, 0.20, 0.30],
            },
        });
    }

    out
}
