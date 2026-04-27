use std::collections::HashMap;

use anyhow::Result;

use crate::assets::AssetResolver;
use crate::defs::ThingDef;
use crate::renderer::{ColoredMeshInput, TexturedMeshInput};
use crate::world::WorldState;

use super::fog_overlay::build_fog_overlays;
use super::lighting_overlay::build_lighting_overlays;
use super::shadow_overlay::build_shadow_overlays;
use super::snow_overlay::build_snow_overlays;

pub struct StaticOverlayInputs {
    pub colored: Vec<ColoredMeshInput>,
    pub textured: Vec<TexturedMeshInput>,
}

pub fn build_static_overlays(
    asset_resolver: &mut AssetResolver,
    thing_defs: &HashMap<String, ThingDef>,
    world: &WorldState,
) -> Result<StaticOverlayInputs> {
    let mut overlays = build_shadow_overlays(thing_defs, world)?;
    overlays.extend(build_lighting_overlays(thing_defs, world)?);
    let mut textured = build_snow_overlays(asset_resolver, world)?;
    textured.extend(build_fog_overlays(asset_resolver, world)?);
    Ok(StaticOverlayInputs {
        colored: overlays,
        textured,
    })
}
