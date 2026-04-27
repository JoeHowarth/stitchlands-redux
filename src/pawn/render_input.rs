//! Helpers that build the texture paths and per-direction data fed into
//! `PawnRenderInput`. They sit between RimWorld defs (`ApparelDef`,
//! body/head/hair/beard graphic paths) and the compose pipeline; the scene
//! builder calls them once per pawn at scene-init time.
//!
//! These lived in `commands::common` historically because the v1 fixture
//! command was the only producer. With the runtime path now also calling
//! into them via `scene::builder`, the helpers belong with the pawn render
//! pipeline rather than the CLI command layer.

use crate::assets::AssetResolver;
use crate::defs::{ApparelDef, ApparelLayerDef, ApparelSkipFlagDef, ApparelWornDirectionDef};

use super::{ApparelLayer, PawnFacing};

pub struct DirectionalTexturePath {
    pub path: String,
    pub data_facing: PawnFacing,
}

/// Try `<path>_<facing>` directional variants in priority order; fall back to
/// the bare `path` if no variant resolves. East/West fall back to each other
/// (RimWorld mirrors the sprite at draw time).
pub fn resolve_directional_tex_path(
    asset_resolver: &mut AssetResolver,
    path: &str,
    facing: PawnFacing,
) -> DirectionalTexturePath {
    if path.ends_with("_north")
        || path.ends_with("_south")
        || path.ends_with("_east")
        || path.ends_with("_west")
    {
        return DirectionalTexturePath {
            path: path.to_string(),
            data_facing: facing,
        };
    }

    let candidates: &[(PawnFacing, &str)] = match facing {
        PawnFacing::North => &[(PawnFacing::North, "_north")],
        PawnFacing::South => &[(PawnFacing::South, "_south")],
        PawnFacing::East => &[(PawnFacing::East, "_east"), (PawnFacing::West, "_west")],
        PawnFacing::West => &[(PawnFacing::West, "_west"), (PawnFacing::East, "_east")],
    };

    for (data_facing, suffix) in candidates {
        let candidate = format!("{path}{suffix}");
        if let Ok(resolved) = asset_resolver.resolve_texture_path(&candidate)
            && !resolved.used_fallback()
        {
            return DirectionalTexturePath {
                path: candidate,
                data_facing: *data_facing,
            };
        }
    }

    // No directional variant found — return the base path as-is so it
    // resolves to the non-directional texture rather than a nonexistent
    // suffixed path.
    DirectionalTexturePath {
        path: path.to_string(),
        data_facing: facing,
    }
}

/// Resolve `worn_graphic` offset/scale for a given facing, applying global
/// and per-direction body-type overrides keyed by `body_type` (lowercased).
pub fn apparel_worn_data_for_facing(
    apparel: &ApparelDef,
    facing: PawnFacing,
    body_type: Option<&str>,
) -> ApparelWornDirectionDef {
    let body_key = body_type.map(|s| s.to_ascii_lowercase());
    let (mut out, directional_overrides) = match facing {
        PawnFacing::North => (
            apparel.worn_graphic.north,
            &apparel.worn_graphic.north_body_overrides,
        ),
        PawnFacing::East => (
            apparel.worn_graphic.east,
            &apparel.worn_graphic.east_body_overrides,
        ),
        PawnFacing::South => (
            apparel.worn_graphic.south,
            &apparel.worn_graphic.south_body_overrides,
        ),
        PawnFacing::West => (
            apparel.worn_graphic.west,
            &apparel.worn_graphic.west_body_overrides,
        ),
    };
    if let Some(body_key) = body_key {
        if let Some(global) = apparel.worn_graphic.global_body_overrides.get(&body_key) {
            if let Some(offset) = global.offset {
                out.offset = offset;
            }
            if let Some(scale) = global.scale {
                out.scale = scale;
            }
        }
        if let Some(local) = directional_overrides.get(&body_key) {
            if let Some(offset) = local.offset {
                out.offset = offset;
            }
            if let Some(scale) = local.scale {
                out.scale = scale;
            }
        }
    }
    out
}

fn apparel_draw_layer_for_facing(apparel: &ApparelDef, facing: PawnFacing) -> Option<f32> {
    match facing {
        PawnFacing::North => apparel.draw_data.north_layer,
        PawnFacing::East => apparel.draw_data.east_layer,
        PawnFacing::South => apparel.draw_data.south_layer,
        PawnFacing::West => apparel.draw_data.west_layer,
    }
}

/// Decode the `<renderSkipFlags>` list into `(skip_hair, skip_beard,
/// has_explicit_flags)`. The third bool tells the compose pipeline whether
/// the pawn has any explicit flags at all (vs none), which affects later
/// layering decisions.
pub fn map_explicit_skip_flags(flags: &Option<Vec<ApparelSkipFlagDef>>) -> (bool, bool, bool) {
    let Some(flags) = flags else {
        return (false, false, false);
    };
    let mut skip_hair = false;
    let mut skip_beard = false;
    for flag in flags {
        match flag {
            ApparelSkipFlagDef::Hair => skip_hair = true,
            ApparelSkipFlagDef::Beard => skip_beard = true,
        }
    }
    (skip_hair, skip_beard, true)
}

/// Build the apparel texture path. For OnSkin/Middle/Shell layers (when not
/// rendered as a pack), prefer a body-type-suffixed variant if it resolves;
/// otherwise fall back to the bare `tex_path`.
pub fn build_apparel_tex_path(
    apparel: &ApparelDef,
    body_def_name: Option<&str>,
    render_as_pack: bool,
    asset_resolver: &mut AssetResolver,
) -> String {
    let layer: ApparelLayer = apparel.layer.into();
    if matches!(
        layer,
        ApparelLayer::OnSkin | ApparelLayer::Middle | ApparelLayer::Shell
    ) && !render_as_pack
        && let Some(body_name) = body_def_name
    {
        let suffixed = format!("{}_{}", apparel.tex_path, body_name);
        if let Ok(resolved) = asset_resolver.resolve_texture_path(&suffixed)
            && !resolved.used_fallback()
        {
            return suffixed;
        }
    }
    apparel.tex_path.clone()
}

/// Resolve the per-facing draw-layer override an apparel needs, falling
/// through to RimWorld's hardcoded constants for shell-rendered-behind-head
/// and pack-anchored layers.
pub fn build_full_apparel_layer_override(
    apparel: &ApparelDef,
    facing: PawnFacing,
    render_as_pack: bool,
) -> Option<f32> {
    apparel_draw_layer_for_facing(apparel, facing).or_else(|| {
        if apparel.layer == ApparelLayerDef::Shell
            && facing == PawnFacing::North
            && !apparel.shell_rendered_behind_head
        {
            Some(88.0)
        } else if render_as_pack {
            match facing {
                PawnFacing::North => Some(93.0),
                PawnFacing::South => Some(-3.0),
                PawnFacing::East | PawnFacing::West => None,
            }
        } else {
            None
        }
    })
}
