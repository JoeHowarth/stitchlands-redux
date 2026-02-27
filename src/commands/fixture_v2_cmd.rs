use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use log::info;

use crate::cell::Cell;
use crate::cli::FixtureV2Cmd;
use crate::defs::{
    ApparelDef, BeardDefRender, BodyTypeDefRender, HairDefRender, HeadTypeDefRender,
};
use crate::pawn::{
    ApparelRenderInput, BeardTypeRenderData, BodyTypeRenderData, HeadTypeRenderData, PawnDrawFlags,
    PawnRenderInput, compose_pawn,
};
use crate::renderer::SpriteParams;
use crate::runtime::v2::{PawnVisualProfile, V2Runtime, V2RuntimeConfig};
use crate::viewer::RenderSprite;
use crate::world::{build_path_grid, issue_move_intent, tick_world, world_from_fixture};

use super::{CommandAction, DispatchContext, LaunchSpec};

pub fn run_fixture_v2(ctx: &mut DispatchContext<'_>, cmd: FixtureV2Cmd) -> Result<CommandAction> {
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
    let sprites = build_world_sprites(ctx, &world)?;
    validate_layer_ownership(&sprites.static_sprites, &sprites.dynamic_sprites)?;
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
        "v2 fixture scene built: scene={} map={}x{} terrain={} things={} blocking_things={} pawns={} static={} dynamic={}",
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

    if !should_run_renderer {
        return Ok(CommandAction::Done);
    }

    let SpriteLayers {
        static_sprites,
        dynamic_sprites,
        pawn_visual_profiles,
    } = sprites;
    let runtime = V2Runtime::new(
        world,
        pawn_visual_profiles,
        V2RuntimeConfig {
            fixed_dt_seconds: cmd.fixed_dt.unwrap_or(1.0 / 60.0),
            compose_config: ctx.compose_config.clone(),
        },
    );
    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites,
        dynamic_sprites,
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
                .resolve_texture_path(ctx.data_dir, &terrain_def.texture_path)
                .with_context(|| {
                    format!(
                        "resolving terrain texture '{}' for '{}'",
                        terrain_def.texture_path, terrain_def.def_name
                    )
                })?;
            static_sprites.push(RenderSprite {
                def_name: format!("Terrain::{}", terrain_def.def_name),
                image: resolved.sprite.image,
                params: SpriteParams {
                    world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -1.0),
                    size: Vec2::new(1.0, 1.0),
                    tint: [1.0, 1.0, 1.0, 1.0],
                },
                used_fallback: resolved.sprite.used_fallback,
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
        let resolved = ctx
            .asset_resolver
            .resolve_thing(ctx.data_dir, thing_def)
            .with_context(|| format!("resolving ThingDef '{}'", thing_def.def_name))?;
        let draw_offset = thing_def.graphic_data.draw_offset;
        static_sprites.push(RenderSprite {
            def_name: format!("Thing::{}", thing_def.def_name),
            image: resolved.sprite.image,
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
            },
            used_fallback: resolved.sprite.used_fallback,
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
        let head = choose_head_def(ctx.defs.head_type_defs, pawn.head.as_deref());
        let hair = choose_hair_def(ctx.defs.hair_defs, pawn.hair.as_deref());
        let beard = choose_beard_def(ctx.defs.beard_defs, pawn.beard.as_deref());

        let apparel_inputs = build_apparel_inputs(ctx.defs.apparel_defs, &pawn.apparel_defs)?;
        let render_input = PawnRenderInput {
            label: pawn.label.clone(),
            facing: pawn.facing,
            world_pos: Vec3::new(pawn.world_pos.x, pawn.world_pos.y, 0.0),
            body_tex_path: body.body_naked_graphic_path.clone(),
            head_tex_path: head.map(|v| v.graphic_path.clone()),
            stump_tex_path: None,
            hair_tex_path: hair.map(|v| v.tex_path.clone()),
            beard_tex_path: beard.map(|v| v.tex_path.clone()),
            body_size: Vec2::ONE,
            head_size: Vec2::ONE,
            stump_size: Vec2::splat(0.8),
            hair_size: head.map(|v| v.hair_mesh_size).unwrap_or(Vec2::ONE),
            beard_size: head.map(|v| v.beard_mesh_size).unwrap_or(Vec2::ONE),
            body_type: BodyTypeRenderData {
                head_offset: body.head_offset,
                body_size_factor: 1.0,
            },
            head_type: head
                .map(|v| HeadTypeRenderData {
                    narrow: v.narrow,
                    narrow_crown_horizontal_offset: 0.0,
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
            present_body_part_groups: vec!["UpperHead".to_string(), "Torso".to_string()],
            hediff_overlays: Vec::new(),
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
                .resolve_texture_path(ctx.data_dir, &node.tex_path)
                .with_context(|| format!("resolving pawn texture '{}'", node.tex_path))?;
            dynamic_sprites.push(RenderSprite {
                def_name: format!("PawnNode::{}", node.id),
                image: resolved.sprite.image,
                params: SpriteParams {
                    world_pos: node.world_pos,
                    size: node.size,
                    tint: node.tint,
                },
                used_fallback: resolved.sprite.used_fallback,
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
    defs: &'a std::collections::HashMap<String, BodyTypeDefRender>,
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

fn choose_head_def<'a>(
    defs: &'a std::collections::HashMap<String, HeadTypeDefRender>,
    preferred: Option<&str>,
) -> Option<&'a HeadTypeDefRender> {
    if let Some(preferred) = preferred
        && let Some(head) = defs.get(preferred)
    {
        return Some(head);
    }
    defs.values().min_by(|a, b| a.def_name.cmp(&b.def_name))
}

fn choose_hair_def<'a>(
    defs: &'a std::collections::HashMap<String, HairDefRender>,
    preferred: Option<&str>,
) -> Option<&'a HairDefRender> {
    if let Some(preferred) = preferred
        && let Some(hair) = defs.get(preferred)
    {
        return Some(hair);
    }
    defs.values().min_by(|a, b| a.def_name.cmp(&b.def_name))
}

fn choose_beard_def<'a>(
    defs: &'a std::collections::HashMap<String, BeardDefRender>,
    preferred: Option<&str>,
) -> Option<&'a BeardDefRender> {
    if let Some(preferred) = preferred
        && let Some(beard) = defs.get(preferred)
    {
        return Some(beard);
    }
    defs.values()
        .filter(|beard| !beard.no_graphic && !beard.tex_path.is_empty())
        .min_by(|a, b| a.def_name.cmp(&b.def_name))
}

fn build_apparel_inputs(
    defs: &std::collections::HashMap<String, ApparelDef>,
    apparel_defs: &[String],
) -> Result<Vec<ApparelRenderInput>> {
    let mut out = Vec::new();
    for def_name in apparel_defs {
        let apparel = defs
            .get(def_name)
            .with_context(|| format!("missing ApparelDef '{}'", def_name))?;
        out.push(ApparelRenderInput {
            label: apparel.def_name.clone(),
            tex_path: apparel.tex_path.clone(),
            layer: apparel.layer.into(),
            explicit_skip_hair: false,
            explicit_skip_beard: false,
            has_explicit_skip_flags: false,
            covers_upper_head: apparel.covers_upper_head,
            covers_full_head: apparel.covers_full_head,
            anchor_to_head: None,
            draw_offset: Vec2::ZERO,
            draw_scale: Vec2::ONE,
            layer_override: None,
            draw_size: apparel.draw_size,
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
    use crate::renderer::SpriteParams;

    fn sprite(def_name: &str) -> RenderSprite {
        RenderSprite {
            def_name: def_name.to_string(),
            image: RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 255])),
            params: SpriteParams {
                world_pos: Vec3::new(0.5, 0.5, 0.0),
                size: Vec2::ONE,
                tint: [1.0, 1.0, 1.0, 1.0],
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
