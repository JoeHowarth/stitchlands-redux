use super::model::{ApparelRenderInput, HediffOverlayInput, PawnDrawFlags, PawnFacing};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ResolvedSkipFlags {
    pub hide_hair: bool,
    pub hide_beard: bool,
}

pub fn resolve_skip_flags(
    draw_flags: PawnDrawFlags,
    apparel: &[ApparelRenderInput],
) -> ResolvedSkipFlags {
    let mut hide_hair = draw_flags.hide_hair;
    let mut hide_beard = draw_flags.hide_beard;

    for item in apparel {
        if item.covers_full_head {
            hide_hair = true;
            hide_beard = true;
            continue;
        }
        if item.covers_upper_head {
            hide_hair = true;
        }
    }

    ResolvedSkipFlags {
        hide_hair,
        hide_beard,
    }
}

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
