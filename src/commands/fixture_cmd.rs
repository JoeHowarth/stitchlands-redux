use std::collections::HashMap;

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use log::{info, warn};

use crate::cell::Cell;
use crate::cli::FixtureCmd;
use crate::defs::{ApparelDef, ApparelLayerDef, BodyTypeDefRender};
use crate::linking::LinkDrawerType;
use crate::pawn::{
    ApparelRenderInput, BeardTypeRenderData, BodyTypeRenderData, HeadTypeRenderData, PawnDrawFlags,
    PawnFacing, PawnRenderInput, compose_pawn,
};
use crate::renderer::{FULL_UV_RECT, SpriteParams};
use crate::runtime::v2::{PawnVisualProfile, V2Runtime, V2RuntimeConfig};
use crate::viewer::RenderSprite;
use crate::world::{build_path_grid, issue_move_intent, tick_world, world_from_fixture};

use super::common::{
    apparel_worn_data_for_facing, build_apparel_tex_path, build_full_apparel_layer_override,
    map_explicit_skip_flags, resolve_directional_tex_path,
};
use super::linking_sprites::{emit_linked_thing_sprites, emit_terrain_edge_sprites};
use super::{CommandAction, DispatchContext, LaunchSpec};

/// RimWorld ships this noise mask as the shared FadeRough / Water alpha
/// variation. Path matches `Verse/TexGame.cs:20`
/// (`ContentFinder<Texture2D>.Get("Other/RoughAlphaAdd")`). The packed
/// resolver matches on basename so the prefix is not load-bearing, but we
/// keep the RimWorld-native path to stay searchable against the decompile.
/// Fallback is a 1x1 gray image; in that case FadeRough edges flatten.
const ROUGH_ALPHA_ADD_PATH: &str = "Other/RoughAlphaAdd";

pub fn run_fixture(ctx: &mut DispatchContext<'_>, cmd: FixtureCmd) -> Result<CommandAction> {
    let (should_run_renderer, render_options, hide_window) = crate::cli::render_runtime(&cmd.view);
    let fixture = crate::fixtures::load_fixture(&cmd.scene)?;
    let mut world = world_from_fixture(&fixture);
    let _ = build_path_grid(&world);
    if let Some(first_pawn_id) = world.pawns().first().map(|pawn| pawn.id) {
        let start = {
            let pawn = world.pawns().iter().find(|pawn| pawn.id == first_pawn_id);
            pawn.map(|pawn| Cell::new(pawn.cell_x, pawn.cell_z))
                .unwrap_or(Cell::new(0, 0))
        };
        let _ = issue_move_intent(&mut world, first_pawn_id, start);
        tick_world(&mut world, 0.0);
    }
    let sprites = build_world_sprites(ctx, &world, !ctx.allow_fallback)?;
    validate_layer_ownership(&sprites.static_sprites, &sprites.dynamic_sprites)?;
    let edge_sprites =
        emit_terrain_edge_sprites(ctx.data_dir, ctx.asset_resolver, &ctx.defs, &world, false)?;
        let noise_image = {
            let resolved = ctx
                .asset_resolver
                .resolve_texture_path(ROUGH_ALPHA_ADD_PATH)
                .with_context(|| format!("resolving noise texture '{ROUGH_ALPHA_ADD_PATH}'"))?;
            if resolved.used_fallback() {
                warn!(
                    "noise texture '{}' not resolved; using 1x1 gray fallback (FadeRough edges will be flat)",
                    ROUGH_ALPHA_ADD_PATH
                );
                crate::renderer::fallback_noise_image()
            } else {
                resolved.image
            }
        };
    let blocking_things = world
        .things()
        .iter()
        .filter(|thing| thing.blocks_movement)
        .count();

    let camera_focus = fixture
        .camera
        .as_ref()
        .map(|camera| Vec2::new(camera.center_x, camera.center_z))
        .or_else(|| {
            Some(Vec2::new(
                world.width() as f32 * 0.5,
                world.height() as f32 * 0.5,
            ))
        });

    info!(
        "fixture scene built: scene={} map={}x{} terrain={} things={} blocking_things={} pawns={} static={} dynamic={}",
        cmd.scene.display(),
        world.width(),
        world.height(),
        world.terrain().len(),
        world.things().len(),
        blocking_things,
        world.pawns().len(),
        sprites.static_sprites.len(),
        sprites.dynamic_sprites.len()
    );

    let SpriteLayers {
        static_sprites,
        dynamic_sprites,
        pawn_visual_profiles,
    } = sprites;
    let mut runtime = V2Runtime::new(
        world,
        pawn_visual_profiles,
        V2RuntimeConfig {
            fixed_dt_seconds: cmd.fixed_dt.unwrap_or(1.0 / 60.0),
            compose_config: ctx.compose_config.clone(),
        },
    );

    if !should_run_renderer {
        let tick_limit = cmd.ticks.unwrap_or(0);
        for _ in 0..tick_limit {
            runtime.tick_once();
        }
        info!(
            "fixture headless ticks complete: ticks={}",
            runtime.tick_count()
        );
        return Ok(CommandAction::Done);
    }

    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites,
        dynamic_sprites,
        edge_sprites,
        noise_image,
        runtime: Some(runtime),
        runtime_tick_limit: cmd.ticks,
        screenshot: cmd.view.screenshot,
        camera_focus,
        render_options,
        hide_window,
        fixed_step: true,
    })))
}

struct SpriteLayers {
    static_sprites: Vec<RenderSprite>,
    dynamic_sprites: Vec<RenderSprite>,
    pawn_visual_profiles: Vec<PawnVisualProfile>,
}

fn build_world_sprites(
    ctx: &mut DispatchContext<'_>,
    world: &crate::world::WorldState,
    strict_missing: bool,
) -> Result<SpriteLayers> {
    let mut static_sprites = Vec::new();
    let mut dynamic_sprites = Vec::new();
    let mut pawn_visual_profiles = Vec::new();

    for z in 0..world.height() {
        for x in 0..world.width() {
            let tile = &world.terrain()[z * world.width() + x];
            let terrain_def = ctx
                .defs
                .terrain_defs
                .get(&tile.terrain_def)
                .with_context(|| format!("missing TerrainDef '{}'", tile.terrain_def))?;
            let resolved = ctx
                .asset_resolver
                .resolve_texture_path(&terrain_def.texture_path)
                .with_context(|| {
                    format!(
                        "resolving terrain texture '{}' for '{}'",
                        terrain_def.texture_path, terrain_def.def_name
                    )
                })?;
            if strict_missing && resolved.used_fallback() {
                anyhow::bail!(
                    "missing terrain texture '{}' for '{}'",
                    terrain_def.texture_path,
                    terrain_def.def_name
                );
            }
            let used_fallback = resolved.used_fallback();
            static_sprites.push(RenderSprite {
                def_name: format!("Terrain::{}", terrain_def.def_name),
                image: resolved.image,
                params: SpriteParams {
                    world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -1.0),
                    size: Vec2::new(1.0, 1.0),
                    tint: [1.0, 1.0, 1.0, 1.0],
                    uv_rect: FULL_UV_RECT,
                },
                used_fallback,
                pawn_id: None,
            });
        }
    }

    let mut things = world.things().to_vec();
    things.sort_by(|a, b| {
        a.cell_z
            .cmp(&b.cell_z)
            .then(a.cell_x.cmp(&b.cell_x))
            .then(a.id.cmp(&b.id))
    });
    for thing in things {
        let thing_def = ctx
            .defs
            .thing_defs
            .get(&thing.def_name)
            .with_context(|| format!("missing ThingDef '{}'", thing.def_name))?;
        if thing_def.graphic_data.link_type != LinkDrawerType::None {
            let linked = emit_linked_thing_sprites(
                ctx.data_dir,
                ctx.asset_resolver,
                &ctx.defs,
                &thing,
                thing_def,
                world,
                strict_missing,
            )?;
            static_sprites.extend(linked);
            continue;
        }
        let resolved = ctx
            .asset_resolver
            .resolve_thing(thing_def, thing.id)
            .with_context(|| format!("resolving ThingDef '{}'", thing_def.def_name))?;
        if strict_missing && resolved.used_fallback() {
            anyhow::bail!(
                "missing thing texture for '{}' ({})",
                thing_def.def_name,
                thing_def.graphic_data.tex_path
            );
        }
        let draw_offset = thing_def.graphic_data.draw_offset;
        let used_fallback = resolved.used_fallback();
        static_sprites.push(RenderSprite {
            def_name: format!("Thing::{}", thing_def.def_name),
            image: resolved.image,
            params: SpriteParams {
                world_pos: Vec3::new(
                    thing.cell_x as f32 + 0.5 + draw_offset.x,
                    thing.cell_z as f32 + 0.5 + draw_offset.z,
                    -0.8 + draw_offset.y * 0.01,
                ),
                size: thing_def.graphic_data.draw_size.max(Vec2::splat(1.1)),
                tint: [
                    thing_def.graphic_data.color.r,
                    thing_def.graphic_data.color.g,
                    thing_def.graphic_data.color.b,
                    thing_def.graphic_data.color.a,
                ],
                uv_rect: FULL_UV_RECT,
            },
            used_fallback,
            pawn_id: None,
        });
    }

    let mut pawns = world.pawns().to_vec();
    pawns.sort_by(|a, b| {
        a.cell_z
            .cmp(&b.cell_z)
            .then(a.cell_x.cmp(&b.cell_x))
            .then(a.id.cmp(&b.id))
    });
    for pawn in pawns {
        let body = choose_body_def(ctx.defs.body_type_defs, pawn.body.as_deref())?;
        let head = choose_def(
            ctx.defs.head_type_defs,
            pawn.head.as_deref(),
            "head",
            |h| &h.def_name,
            |_| true,
        );
        let hair = choose_def(
            ctx.defs.hair_defs,
            pawn.hair.as_deref(),
            "hair",
            |h| &h.def_name,
            |_| true,
        );
        let beard = choose_def(
            ctx.defs.beard_defs,
            pawn.beard.as_deref(),
            "beard",
            |b| &b.def_name,
            |b| !b.no_graphic && !b.tex_path.is_empty(),
        );

        let facing = pawn.facing;

        // Resolve directional texture paths for body/head/hair/beard
        let body_directional =
            resolve_directional_tex_path(ctx.asset_resolver, &body.body_naked_graphic_path, facing);
        let head_tex_path = head.map(|h| {
            resolve_directional_tex_path(ctx.asset_resolver, &h.graphic_path, facing).path
        });
        let hair_tex_path = hair.map(|h| {
            resolve_directional_tex_path(ctx.asset_resolver, &h.tex_path, facing).path
        });
        let beard_tex_path = beard.map(|b| {
            resolve_directional_tex_path(ctx.asset_resolver, &b.tex_path, facing).path
        });

        let apparel_inputs = build_apparel_inputs(
            ctx.defs.apparel_defs,
            &pawn.apparel_defs,
            Some(&body.def_name),
            facing,
            ctx.asset_resolver,
        )?;

        let render_input = PawnRenderInput {
            label: pawn.label.clone(),
            facing,
            world_pos: Vec3::new(pawn.world_pos.x, pawn.world_pos.y, 0.0),
            body_tex_path: body_directional.path,
            head_tex_path,
            stump_tex_path: None,
            hair_tex_path,
            beard_tex_path,
            body_type: BodyTypeRenderData {
                head_offset: body.head_offset,
            },
            head_type: head
                .map(|v| HeadTypeRenderData {
                    narrow: v.narrow,
                    beard_offset: v.beard_offset,
                    beard_offset_x_east: v.beard_offset_x_east,
                })
                .unwrap_or_default(),
            beard_type: beard
                .map(|v| BeardTypeRenderData {
                    offset_narrow_east: v.offset_narrow_east,
                    offset_narrow_south: v.offset_narrow_south,
                })
                .unwrap_or_default(),
            tint: [1.0, 1.0, 1.0, 1.0],
            apparel: apparel_inputs,
            draw_flags: PawnDrawFlags::NONE,
        };
        pawn_visual_profiles.push(PawnVisualProfile {
            pawn_id: pawn.id,
            base_render_input: render_input.clone(),
        });

        let composed = compose_pawn(&render_input, &ctx.compose_config);
        for node in composed.nodes {
            let resolved = ctx
                .asset_resolver
                .resolve_texture_path(&node.tex_path)
                .with_context(|| format!("resolving pawn texture '{}'", node.tex_path))?;
            if strict_missing && resolved.used_fallback() {
                anyhow::bail!("missing pawn node texture: {}", node.tex_path);
            }
            let used_fallback = resolved.used_fallback();
            dynamic_sprites.push(RenderSprite {
                def_name: format!("PawnNode::{}", node.id),
                image: resolved.image,
                params: SpriteParams {
                    world_pos: node.world_pos,
                    size: node.size,
                    tint: node.tint,
                    uv_rect: FULL_UV_RECT,
                },
                used_fallback,
                pawn_id: Some(pawn.id),
            });
        }
    }

    Ok(SpriteLayers {
        static_sprites,
        dynamic_sprites,
        pawn_visual_profiles,
    })
}

fn choose_body_def<'a>(
    defs: &'a HashMap<String, BodyTypeDefRender>,
    preferred: Option<&str>,
) -> Result<&'a BodyTypeDefRender> {
    if let Some(preferred) = preferred
        && let Some(body) = defs.get(preferred)
    {
        return Ok(body);
    }
    defs.values()
        .min_by(|a, b| a.def_name.cmp(&b.def_name))
        .context("no BodyTypeDefRender entries are available")
}

fn choose_def<'a, T>(
    defs: &'a HashMap<String, T>,
    preferred: Option<&str>,
    kind: &str,
    key_of: impl Fn(&T) -> &str,
    eligible: impl Fn(&T) -> bool,
) -> Option<&'a T> {
    if let Some(name) = preferred {
        if let Some(value) = defs.get(name) {
            return Some(value);
        }
        warn!("preferred {kind} def '{name}' not found, falling back");
    }
    defs.values()
        .filter(|v| eligible(v))
        .min_by_key(|v| key_of(v))
}

fn build_apparel_inputs(
    defs: &HashMap<String, ApparelDef>,
    apparel_defs: &[String],
    body_def_name: Option<&str>,
    facing: PawnFacing,
    asset_resolver: &mut crate::assets::AssetResolver,
) -> Result<Vec<ApparelRenderInput>> {
    let mut out = Vec::new();
    for def_name in apparel_defs {
        let apparel = defs
            .get(def_name)
            .with_context(|| format!("missing ApparelDef '{}'", def_name))?;

        // Match RimWorld's RenderAsPack(): only Belt items can render as pack,
        // gated by renderUtilityAsPack. Non-Belt items never render as pack.
        let render_as_pack = if matches!(apparel.layer, ApparelLayerDef::Belt) {
            apparel.worn_graphic.render_utility_as_pack
        } else {
            false
        };

        // Resolve body-type suffixed texture path
        let tex_path = build_apparel_tex_path(
            apparel,
            body_def_name,
            render_as_pack,
            asset_resolver,
        );

        // Resolve directional texture
        let directional = resolve_directional_tex_path(asset_resolver, &tex_path, facing);
        let tex_path = directional.path;

        // Worn data (offset/scale) with body overrides
        let worn_data =
            apparel_worn_data_for_facing(apparel, directional.data_facing, body_def_name);

        let (explicit_skip_hair, explicit_skip_beard, has_explicit_skip_flags) =
            map_explicit_skip_flags(&apparel.render_skip_flags);

        let layer_override = build_full_apparel_layer_override(apparel, facing, render_as_pack);

        let anchor_to_head = match apparel.parent_tag_def.as_deref() {
            Some("ApparelHead") => Some(true),
            Some("ApparelBody") => Some(false),
            _ => None,
        };

        out.push(ApparelRenderInput {
            label: apparel.def_name.clone(),
            tex_path,
            layer: apparel.layer.into(),
            explicit_skip_hair,
            explicit_skip_beard,
            has_explicit_skip_flags,
            covers_upper_head: apparel.covers_upper_head,
            covers_full_head: apparel.covers_full_head,
            anchor_to_head,
            pack_offset: worn_data.offset,
            pack_scale: worn_data.scale,
            render_as_pack,
            layer_override,
            tint: [
                apparel.color.r,
                apparel.color.g,
                apparel.color.b,
                apparel.color.a,
            ],
        });
    }
    Ok(out)
}

fn validate_layer_ownership(
    static_sprites: &[RenderSprite],
    dynamic_sprites: &[RenderSprite],
) -> Result<()> {
    let static_invalid: Vec<&str> = static_sprites
        .iter()
        .filter_map(|sprite| {
            let name = sprite.def_name.as_str();
            if name.starts_with("Terrain::") || name.starts_with("Thing::") {
                None
            } else {
                Some(name)
            }
        })
        .collect();
    if !static_invalid.is_empty() {
        anyhow::bail!(
            "v2 layer ownership violation: static layer contains non-terrain/non-thing sprites: {}",
            static_invalid.join(", ")
        );
    }

    let dynamic_invalid: Vec<&str> = dynamic_sprites
        .iter()
        .filter_map(|sprite| {
            let name = sprite.def_name.as_str();
            if name.starts_with("PawnNode::") {
                None
            } else {
                Some(name)
            }
        })
        .collect();
    if !dynamic_invalid.is_empty() {
        anyhow::bail!(
            "v2 layer ownership violation: dynamic layer contains non-pawn sprites: {}",
            dynamic_invalid.join(", ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3};
    use image::{Rgba, RgbaImage};

    use super::{RenderSprite, validate_layer_ownership};
    use crate::renderer::{FULL_UV_RECT, SpriteParams};

    fn sprite(def_name: &str) -> RenderSprite {
        RenderSprite {
            def_name: def_name.to_string(),
            image: RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 255])),
            params: SpriteParams {
                world_pos: Vec3::new(0.5, 0.5, 0.0),
                size: Vec2::ONE,
                tint: [1.0, 1.0, 1.0, 1.0],
                uv_rect: FULL_UV_RECT,
            },
            used_fallback: false,
            pawn_id: None,
        }
    }

    #[test]
    fn layer_ownership_accepts_expected_partition() {
        let static_sprites = vec![sprite("Terrain::Soil"), sprite("Thing::ChunkSlagSteel")];
        let dynamic_sprites = vec![
            sprite("PawnNode::PawnA::Body"),
            sprite("PawnNode::PawnA::Head"),
        ];
        assert!(validate_layer_ownership(&static_sprites, &dynamic_sprites).is_ok());
    }

    #[test]
    fn layer_ownership_rejects_pawn_in_static() {
        let static_sprites = vec![sprite("Terrain::Soil"), sprite("PawnNode::PawnA::Body")];
        let dynamic_sprites = vec![sprite("PawnNode::PawnA::Head")];
        let err =
            validate_layer_ownership(&static_sprites, &dynamic_sprites).expect_err("should fail");
        assert!(err.to_string().contains("static layer"));
    }

    #[test]
    fn layer_ownership_rejects_thing_in_dynamic() {
        let static_sprites = vec![sprite("Terrain::Soil")];
        let dynamic_sprites = vec![
            sprite("PawnNode::PawnA::Head"),
            sprite("Thing::ChunkSlagSteel"),
        ];
        let err =
            validate_layer_ownership(&static_sprites, &dynamic_sprites).expect_err("should fail");
        assert!(err.to_string().contains("dynamic layer"));
    }
}
