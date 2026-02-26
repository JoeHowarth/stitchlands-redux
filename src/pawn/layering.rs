use super::model::{ApparelLayer, LayeringProfile, PawnFacing};

pub fn facing_x_offset(facing: PawnFacing) -> f32 {
    match facing {
        PawnFacing::East => 0.05,
        PawnFacing::West => -0.05,
        PawnFacing::North | PawnFacing::South => 0.0,
    }
}

pub fn apparel_z(profile: LayeringProfile, layer: ApparelLayer, stack_index: usize) -> f32 {
    let base = if matches!(layer, ApparelLayer::Overhead | ApparelLayer::EyeCover) {
        profile.apparel_head_base_z
    } else {
        profile.apparel_body_base_z
    };
    base + layer.draw_order() as f32 * profile.apparel_step_z + stack_index as f32 * 0.0001
}
