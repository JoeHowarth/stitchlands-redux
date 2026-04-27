//! Static-overlay scene assembly: fog of war, snow, sun shadows, glow
//! lighting. Overlay builders live here (formerly under `commands/`)
//! because runtime scene assembly calls them every frame; commands is now a
//! pure caller via `crate::scene::builder::build_scene`.

use std::collections::HashMap;

use anyhow::Result;

use crate::defs::ThingDef;
use crate::scene::{ColoredMeshInput, TexturedMeshInput};
use crate::world::WorldState;

pub mod fog;
pub mod glow_grid;
pub mod lighting;
pub mod shadow;
pub mod sky_shadow;
pub mod snow;
mod solid_mesh;

use fog::build_fog_overlays;
use lighting::build_lighting_overlays;
use shadow::build_shadow_overlays;
use snow::build_snow_overlays;

pub struct StaticOverlayInputs {
    pub colored: Vec<ColoredMeshInput>,
    pub textured: Vec<TexturedMeshInput>,
}

pub fn build_static_overlays(
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<StaticOverlayInputs> {
    let mut overlays = build_shadow_overlays(thing_defs, world)?;
    overlays.extend(build_lighting_overlays(thing_defs, world)?);
    let mut textured = build_snow_overlays(world)?;
    textured.extend(build_fog_overlays(world)?);
    Ok(StaticOverlayInputs {
        colored: overlays,
        textured,
    })
}
