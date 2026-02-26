use super::model::{ApparelRenderInput, PawnDrawFlags};

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
