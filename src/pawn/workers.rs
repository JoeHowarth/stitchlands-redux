use glam::Vec2;

use super::graph::AnchorKind;
use super::model::{
    ApparelLayer, BeardTypeRenderData, BodyTypeRenderData, HeadTypeRenderData, LayeringProfile,
    PawnFacing,
};

pub fn facing_x_offset(facing: PawnFacing) -> f32 {
    match facing {
        PawnFacing::East => 0.05,
        PawnFacing::West => -0.05,
        PawnFacing::North | PawnFacing::South => 0.0,
    }
}

pub fn anchor_offset(anchor: AnchorKind, facing: PawnFacing, body: BodyTypeRenderData) -> Vec2 {
    match anchor {
        AnchorKind::Body => Vec2::ZERO,
        AnchorKind::Head => base_head_offset(facing, body),
    }
}

pub fn beard_extra_offset(
    facing: PawnFacing,
    head_type: HeadTypeRenderData,
    beard_type: BeardTypeRenderData,
    _layering: LayeringProfile,
) -> Vec2 {
    let mut out = Vec2::ZERO;
    match facing {
        PawnFacing::East => out.x += head_type.beard_offset_x_east,
        PawnFacing::West => out.x -= head_type.beard_offset_x_east,
        PawnFacing::North | PawnFacing::South => {}
    }
    out += Vec2::new(head_type.beard_offset.x, head_type.beard_offset.z);
    if head_type.narrow && !matches!(facing, PawnFacing::North) {
        match facing {
            PawnFacing::South => {
                out += Vec2::new(
                    beard_type.offset_narrow_south.x,
                    beard_type.offset_narrow_south.z,
                )
            }
            PawnFacing::East => {
                out += Vec2::new(
                    beard_type.offset_narrow_east.x,
                    beard_type.offset_narrow_east.z,
                )
            }
            PawnFacing::West => {
                out += Vec2::new(
                    -beard_type.offset_narrow_east.x,
                    beard_type.offset_narrow_east.z,
                )
            }
            PawnFacing::North => {}
        }
    }
    out
}

pub fn apparel_offset(layer: ApparelLayer, layering: LayeringProfile) -> Vec2 {
    if matches!(layer, ApparelLayer::Overhead | ApparelLayer::EyeCover) {
        Vec2::new(0.0, layering.apparel_head_y_offset)
    } else {
        Vec2::ZERO
    }
}

pub fn apparel_z(profile: LayeringProfile, layer: ApparelLayer, stack_index: usize) -> f32 {
    let base = if matches!(layer, ApparelLayer::Overhead | ApparelLayer::EyeCover) {
        profile.apparel_head_base_z
    } else {
        profile.apparel_body_base_z
    };
    base + stack_index as f32 * profile.apparel_step_z
}

pub fn layer_to_z_delta(layer: f32) -> f32 {
    layer.clamp(-10.0, 100.0) * 0.000_365_853_7
}

fn base_head_offset(facing: PawnFacing, body: BodyTypeRenderData) -> Vec2 {
    let offset = body.head_offset;
    match facing {
        PawnFacing::North | PawnFacing::South => Vec2::new(0.0, offset.y),
        PawnFacing::East => Vec2::new(offset.x, offset.y),
        PawnFacing::West => Vec2::new(-offset.x, offset.y),
    }
}
