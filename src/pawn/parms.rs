use super::model::{ApparelRenderInput, PawnDrawFlags, PawnFacing};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RenderSkipFlag {
    Hair,
    Beard,
    Eyes,
}

#[derive(Debug, Clone)]
pub struct PawnDrawParms {
    pub facing: PawnFacing,
    pub draw_flags: PawnDrawFlags,
    pub skip_flags: Vec<RenderSkipFlag>,
}

impl PawnDrawParms {
    pub fn from_inputs(
        facing: PawnFacing,
        draw_flags: PawnDrawFlags,
        apparel: &[ApparelRenderInput],
    ) -> Self {
        let mut skip_flags = Vec::new();
        if draw_flags.hide_hair {
            skip_flags.push(RenderSkipFlag::Hair);
        }
        if draw_flags.hide_beard {
            skip_flags.push(RenderSkipFlag::Beard);
        }
        for item in apparel {
            if item.covers_full_head {
                push_unique(&mut skip_flags, RenderSkipFlag::Hair);
                push_unique(&mut skip_flags, RenderSkipFlag::Beard);
                push_unique(&mut skip_flags, RenderSkipFlag::Eyes);
            } else if item.covers_upper_head {
                push_unique(&mut skip_flags, RenderSkipFlag::Hair);
            }
        }
        Self {
            facing,
            draw_flags,
            skip_flags,
        }
    }

    pub fn skip(&self, flag: RenderSkipFlag) -> bool {
        self.skip_flags.contains(&flag)
    }
}

fn push_unique(out: &mut Vec<RenderSkipFlag>, flag: RenderSkipFlag) {
    if !out.contains(&flag) {
        out.push(flag);
    }
}
