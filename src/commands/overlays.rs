use std::collections::HashMap;

use anyhow::Result;

use crate::defs::ThingDef;
use crate::renderer::ColoredMeshInput;
use crate::world::WorldState;

use super::lighting_overlay::build_lighting_overlays;
use super::shadow_overlay::build_shadow_overlays;

pub fn build_static_overlays(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<Vec<ColoredMeshInput>> {
    let mut overlays = build_shadow_overlays(thing_defs, world)?;
    overlays.extend(build_lighting_overlays(thing_defs, world)?);
    Ok(overlays)
}
