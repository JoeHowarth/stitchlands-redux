use super::model::{HediffOverlayInput, PawnFacing};

pub fn should_draw_hediff_overlay(
    overlay: &HediffOverlayInput,
    facing: PawnFacing,
    present_body_part_groups: &[String],
) -> bool {
    if let Some(required_group) = &overlay.required_body_part_group
        && !present_body_part_groups
            .iter()
            .any(|g| g.eq_ignore_ascii_case(required_group))
    {
        return false;
    }

    if let Some(visible_facing) = &overlay.visible_facing
        && !visible_facing.contains(&facing)
    {
        return false;
    }

    true
}
