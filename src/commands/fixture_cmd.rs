use std::path::PathBuf;

use anyhow::Result;
use glam::Vec2;
use log::{info, warn};

use crate::cli::{AuditCmd, FixtureCmd};
use crate::defs::ApparelLayerDef;
use crate::fixtures::{
    CameraSpec, MapSpec, PawnSpawn, SceneFixture, TerrainCell, ThingSpawn,
};
use crate::pawn::PawnFacing;
use crate::runtime::v2::{V2Runtime, V2RuntimeConfig};
use crate::scene::generate_fixture_map;
use crate::world::world_from_fixture;

use super::common::{body_head_compatible, select_fixture_apparel_names};
use super::fixture_v2_cmd::build_world_sprites;
use super::{CommandAction, DispatchContext, LaunchSpec};

pub fn run_fixture(ctx: &mut DispatchContext<'_>, mode: FixtureCmd) -> Result<CommandAction> {
    if let FixtureCmd::V2(args) = mode {
        return super::fixture_v2_cmd::run_fixture_v2(ctx, args);
    }

    let (fixture_args, is_pawn) = match mode {
        FixtureCmd::V1(args) => (args, false),
        FixtureCmd::Pawn(args) => (args, true),
        FixtureCmd::V2(_) => unreachable!("v2 handled above"),
    };
    let (should_run_renderer, render_options, hide_window) =
        crate::cli::render_runtime(&fixture_args.view);

    let (width, height, pawn_focus_only, pawn_count) = if is_pawn {
        (
            fixture_args.map_width.clamp(8, 18),
            fixture_args.map_height.clamp(8, 18),
            true,
            1,
        )
    } else {
        (fixture_args.map_width, fixture_args.map_height, false, 6)
    };

    let scene_fixture = generate_v1_scene_fixture(
        ctx,
        width,
        height,
        pawn_count,
        fixture_args.pawn_fixture_variant,
        pawn_focus_only,
        false,
    )?;

    let strict_missing = !ctx.allow_fallback;
    let world = world_from_fixture(&scene_fixture);
    let sprites = build_world_sprites(ctx, &world, strict_missing)?;

    if pawn_focus_only {
        validate_pawn_focus(&sprites, fixture_args.pawn_fixture_variant)?;
    }

    if let Some(path) = &fixture_args.dump_pawn_trace {
        dump_trace(ctx, &world, path)?;
    }

    let camera_focus = scene_fixture
        .camera
        .as_ref()
        .map(|cam| Vec2::new(cam.center_x, cam.center_z));

    if !should_run_renderer {
        return Ok(CommandAction::Done);
    }

    let runtime = V2Runtime::new(
        world,
        sprites.pawn_visual_profiles,
        V2RuntimeConfig {
            fixed_dt_seconds: 1.0 / 60.0,
            compose_config: ctx.compose_config.clone(),
        },
    );
    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites: sprites.static_sprites,
        dynamic_sprites: sprites.dynamic_sprites,
        runtime: Some(runtime),
        runtime_tick_limit: None,
        screenshot: fixture_args.view.screenshot,
        camera_focus,
        render_options,
        hide_window,
        fixed_step: true,
    })))
}

pub fn run_audit(ctx: &mut DispatchContext<'_>, audit: AuditCmd) -> Result<CommandAction> {
    let (should_run_renderer, render_options, hide_window) =
        crate::cli::render_runtime(&audit.view);

    let scene_fixture = generate_v1_scene_fixture(
        ctx,
        audit.map_width.max(24),
        audit.map_height.max(24),
        audit.pawn_count.clamp(6, 20),
        audit.pawn_fixture_variant,
        false,
        true,
    )?;

    let strict_missing = !ctx.allow_fallback;
    let world = world_from_fixture(&scene_fixture);
    let sprites = build_world_sprites(ctx, &world, strict_missing)?;

    if let Some(path) = &audit.dump_pawn_trace {
        dump_trace(ctx, &world, path)?;
    }

    let camera_focus = scene_fixture
        .camera
        .as_ref()
        .map(|cam| Vec2::new(cam.center_x, cam.center_z));

    if !should_run_renderer {
        return Ok(CommandAction::Done);
    }

    let runtime = V2Runtime::new(
        world,
        sprites.pawn_visual_profiles,
        V2RuntimeConfig {
            fixed_dt_seconds: 1.0 / 60.0,
            compose_config: ctx.compose_config.clone(),
        },
    );
    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites: sprites.static_sprites,
        dynamic_sprites: sprites.dynamic_sprites,
        runtime: Some(runtime),
        runtime_tick_limit: None,
        screenshot: audit.view.screenshot,
        camera_focus,
        render_options,
        hide_window,
        fixed_step: true,
    })))
}

/// Build a SceneFixture procedurally, filtering all selections to decodable
/// textures just like the old v1 monolith did.
fn generate_v1_scene_fixture(
    ctx: &mut DispatchContext<'_>,
    width: usize,
    height: usize,
    pawn_count: usize,
    pawn_fixture_variant: usize,
    pawn_focus_only: bool,
    pawn_audit_mode: bool,
) -> Result<SceneFixture> {
    let defs = &ctx.defs;

    // --- Terrain: sorted, filtered to decodable, take first 3 ---
    let mut terrain_rows: Vec<_> = defs.terrain_defs.values().collect();
    terrain_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut chosen_terrain: Vec<&str> = Vec::new();
    for terrain in &terrain_rows {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ctx.data_dir, &terrain.texture_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        chosen_terrain.push(&terrain.def_name);
        if chosen_terrain.len() >= 3 {
            break;
        }
    }
    if chosen_terrain.len() < 3 {
        anyhow::bail!("v1 fixture needs at least 3 decodable terrain defs");
    }
    let terrain_arr = [chosen_terrain[0], chosen_terrain[1], chosen_terrain[2]];

    // --- Things: preferred list, filtered to decodable ---
    let preferred_things = [
        "Steel",
        "ChunkSlagSteel",
        "Plasteel",
        "WoodLog",
        "ComponentIndustrial",
    ];
    let thing_names: Vec<String> = if pawn_focus_only || pawn_audit_mode {
        Vec::new()
    } else {
        let mut out = Vec::new();
        for name in preferred_things {
            if let Some(def) = defs.thing_defs.get(name) {
                let resolved = ctx.asset_resolver.resolve_thing(ctx.data_dir, def)?;
                if resolved.sprite.used_fallback {
                    continue;
                }
                out.push(name.to_string());
            }
        }
        out
    };
    if thing_names.is_empty() && !pawn_focus_only && !pawn_audit_mode {
        anyhow::bail!("v1 fixture needs at least one decodable ThingDef");
    }

    // --- Body types: sorted, filtered to decodable ---
    let mut body_rows_all: Vec<_> = defs.body_type_defs.values().collect();
    body_rows_all.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut body_rows = Vec::new();
    for body in &body_rows_all {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ctx.data_dir, &body.body_naked_graphic_path)?;
        if !resolved.sprite.used_fallback {
            body_rows.push(*body);
        }
    }
    if body_rows.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable pawn body texture");
    }

    // --- Head types: sorted, filtered to decodable ---
    let mut head_rows_all: Vec<_> = defs.head_type_defs.values().collect();
    head_rows_all.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut head_rows = Vec::new();
    for head in &head_rows_all {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ctx.data_dir, &head.graphic_path)?;
        if !resolved.sprite.used_fallback {
            head_rows.push(*head);
        }
    }

    // --- Hair: sorted, filtered to decodable ---
    let mut hair_rows_all: Vec<_> = defs.hair_defs.values().collect();
    hair_rows_all.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut hair_rows = Vec::new();
    for hair in &hair_rows_all {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ctx.data_dir, &hair.tex_path)?;
        if !resolved.sprite.used_fallback {
            hair_rows.push(*hair);
        }
    }

    // --- Beard: sorted, filtered to decodable + no anchors ---
    let mut beard_rows_all: Vec<_> = defs
        .beard_defs
        .values()
        .filter(|b| !b.no_graphic && !b.tex_path.is_empty())
        .filter(|b| {
            let key = b.def_name.to_ascii_lowercase();
            let tex = b.tex_path.to_ascii_lowercase();
            !key.contains("anchor") && !tex.contains("beardanchor")
        })
        .collect();
    beard_rows_all.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut beard_rows = Vec::new();
    for beard in &beard_rows_all {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ctx.data_dir, &beard.tex_path)?;
        if !resolved.sprite.used_fallback {
            beard_rows.push(*beard);
        }
    }

    // --- Apparel: sorted, categorized by layer, filtered to decodable ---
    let mut apparel_rows: Vec<_> = defs.apparel_defs.values().collect();
    apparel_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut body_layer_apparel = Vec::new();
    let mut shell_layer_apparel = Vec::new();
    let mut head_layer_apparel = Vec::new();
    for apparel in &apparel_rows {
        let resolved = ctx
            .asset_resolver
            .resolve_texture_path(ctx.data_dir, &apparel.tex_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        let is_body = matches!(
            apparel.layer,
            ApparelLayerDef::OnSkin | ApparelLayerDef::Middle
        );
        let is_shellish = matches!(
            apparel.layer,
            ApparelLayerDef::Shell | ApparelLayerDef::Belt
        );
        let is_head = matches!(
            apparel.layer,
            ApparelLayerDef::Overhead | ApparelLayerDef::EyeCover
        );
        if is_body {
            body_layer_apparel.push(*apparel);
        }
        if is_shellish {
            shell_layer_apparel.push(*apparel);
        }
        if is_head {
            head_layer_apparel.push(*apparel);
        }
    }

    let decodable_apparel_layers = usize::from(!body_layer_apparel.is_empty())
        + usize::from(!shell_layer_apparel.is_empty())
        + usize::from(!head_layer_apparel.is_empty());
    if decodable_apparel_layers == 0 {
        warn!("v1 fixture found no decodable apparel layers; pawns will be unclothed");
    } else if pawn_focus_only && decodable_apparel_layers < 2 {
        anyhow::bail!(
            "pawn fixture requires at least two decodable apparel layers (body + shell/head)"
        );
    }

    // --- Layout ---
    let layout_pawn_count = if pawn_focus_only {
        1
    } else if pawn_audit_mode {
        pawn_count
    } else {
        body_rows.len().min(6)
    };

    let mut map = generate_fixture_map(
        width.max(8),
        height.max(8),
        terrain_arr,
        &thing_names,
        layout_pawn_count,
    );

    if pawn_audit_mode {
        let cols = 5usize;
        let spacing = 4i32;
        let rows = map.pawns.len().div_ceil(cols).max(1);
        let span_x = (cols.saturating_sub(1) as i32) * spacing;
        let span_z = (rows.saturating_sub(1) as i32) * spacing;
        let origin_x = ((map.width as i32 - 1 - span_x) / 2).max(0);
        let origin_z = ((map.height as i32 - 1 - span_z) / 2).max(0);
        for (i, pawn) in map.pawns.iter_mut().enumerate() {
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            pawn.cell_x = (origin_x + col * spacing).min(map.width.saturating_sub(1) as i32);
            pawn.cell_z = (origin_z + row * spacing).min(map.height.saturating_sub(1) as i32);
        }
    }

    // Convert terrain to TerrainCell vec
    let terrain: Vec<TerrainCell> = map
        .terrain
        .iter()
        .map(|name| TerrainCell {
            terrain_def: name.clone(),
        })
        .collect();

    // Convert things to ThingSpawn vec
    let things: Vec<ThingSpawn> = map
        .things
        .iter()
        .map(|thing| ThingSpawn {
            def_name: thing.def_name.clone(),
            cell_x: thing.cell_x,
            cell_z: thing.cell_z,
            blocks_movement: false,
        })
        .collect();

    // --- Build PawnSpawn for each map pawn ---
    let pawns: Vec<PawnSpawn> = map
        .pawns
        .iter()
        .enumerate()
        .map(|(pawn_index, map_pawn)| {
            let body_def = &body_rows[pawn_index % body_rows.len()];
            let body_tex = &body_def.body_naked_graphic_path;

            let facing = if pawn_focus_only {
                PawnFacing::South
            } else {
                map_pawn.facing
            };

            // Head selection: filter by body compatibility
            let compatible_heads: Vec<_> = head_rows
                .iter()
                .copied()
                .filter(|h| body_head_compatible(body_tex, &h.graphic_path))
                .collect();
            let head_pool = if compatible_heads.is_empty() {
                &head_rows
            } else {
                &compatible_heads
            };

            // Hair selection
            let hair_name = if hair_rows.is_empty() {
                None
            } else {
                Some(
                    hair_rows[(pawn_fixture_variant / 5) % hair_rows.len()]
                        .def_name
                        .clone(),
                )
            };

            // Beard selection
            let beard_name = if beard_rows.is_empty() {
                None
            } else {
                Some(
                    beard_rows[(pawn_fixture_variant / 7) % beard_rows.len()]
                        .def_name
                        .clone(),
                )
            };

            // If we have a beard, prefer male heads
            let final_head_pool: Vec<_> = if beard_name.is_some() {
                let male_heads: Vec<_> = head_pool
                    .iter()
                    .copied()
                    .filter(|h| h.graphic_path.to_ascii_lowercase().contains("/male/"))
                    .collect();
                if male_heads.is_empty() {
                    head_pool.to_vec()
                } else {
                    male_heads
                }
            } else {
                head_pool.to_vec()
            };

            let head_name = if final_head_pool.is_empty() {
                None
            } else {
                Some(
                    final_head_pool[pawn_fixture_variant % final_head_pool.len()]
                        .def_name
                        .clone(),
                )
            };

            let apparel_names = select_fixture_apparel_names(
                pawn_index,
                pawn_fixture_variant,
                pawn_focus_only,
                &body_layer_apparel,
                &shell_layer_apparel,
                &head_layer_apparel,
            );

            PawnSpawn {
                cell_x: map_pawn.cell_x,
                cell_z: map_pawn.cell_z,
                label: Some(map_pawn.label.clone()),
                body: Some(body_def.def_name.clone()),
                head: head_name,
                hair: hair_name,
                beard: beard_name,
                apparel_defs: apparel_names,
                facing,
            }
        })
        .collect();

    let camera = if pawn_audit_mode {
        Some(CameraSpec {
            center_x: map.width as f32 * 0.5,
            center_z: map.height as f32 * 0.5,
            zoom: 1.0,
        })
    } else if let Some(thing) = map.things.first() {
        Some(CameraSpec {
            center_x: thing.cell_x as f32 + 0.5,
            center_z: thing.cell_z as f32 + 0.5,
            zoom: 1.0,
        })
    } else if let Some(pawn) = map.pawns.first() {
        Some(CameraSpec {
            center_x: pawn.cell_x as f32 + 0.5,
            center_z: pawn.cell_z as f32 + 0.5,
            zoom: 1.0,
        })
    } else {
        Some(CameraSpec {
            center_x: map.width as f32 * 0.5,
            center_z: map.height as f32 * 0.5,
            zoom: 1.0,
        })
    };

    info!(
        "generated v1 scene fixture: map={}x{} terrain_cells={} things={} pawns={} variant={}",
        map.width,
        map.height,
        terrain.len(),
        things.len(),
        pawns.len(),
        pawn_fixture_variant,
    );

    Ok(SceneFixture {
        schema_version: 2,
        map: MapSpec {
            width: map.width,
            height: map.height,
            terrain,
        },
        things,
        pawns,
        camera,
    })
}

fn validate_pawn_focus(
    sprites: &super::fixture_v2_cmd::SpriteLayers,
    pawn_fixture_variant: usize,
) -> Result<()> {
    for profile in &sprites.pawn_visual_profiles {
        let composed = crate::pawn::compose_pawn(
            &profile.base_render_input,
            &crate::pawn::PawnComposeConfig::default(),
        );
        if let Some(head_body_delta) = measure_head_body_delta_y(&composed.nodes)
            && head_body_delta <= 0.0
        {
            anyhow::bail!(
                "upside-down pawn composition detected for variant {pawn_fixture_variant}: head_body_delta_y={head_body_delta:.3}"
            );
        }
        if let Some(violations) = validate_basic_pawn_layering(&composed.nodes) {
            anyhow::bail!(
                "invalid pawn layering for variant {pawn_fixture_variant}: {}",
                violations.join("; ")
            );
        }
    }
    Ok(())
}

fn measure_head_body_delta_y(nodes: &[crate::pawn::tree::PawnNode]) -> Option<f32> {
    let body_y = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Body))
        .map(|n| n.world_pos.y)?;
    let head_y = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Head))
        .map(|n| n.world_pos.y)
        .or_else(|| {
            nodes
                .iter()
                .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Stump))
                .map(|n| n.world_pos.y)
        })?;
    Some(head_y - body_y)
}

fn validate_basic_pawn_layering(nodes: &[crate::pawn::tree::PawnNode]) -> Option<Vec<String>> {
    let body_z = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Body))
        .map(|n| n.z)?;
    let head_z = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Head))
        .map(|n| n.z)
        .or_else(|| {
            nodes
                .iter()
                .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Stump))
                .map(|n| n.z)
        })?;

    let mut violations = Vec::new();
    if head_z <= body_z {
        violations.push(format!("head_z({head_z:.6}) <= body_z({body_z:.6})"));
    }

    if let Some(hair_z) = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Hair))
        .map(|n| n.z)
        && hair_z <= head_z
    {
        violations.push(format!("hair_z({hair_z:.6}) <= head_z({head_z:.6})"));
    }

    if let Some(beard_z) = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Beard))
        .map(|n| n.z)
        && beard_z <= body_z
    {
        violations.push(format!("beard_z({beard_z:.6}) <= body_z({body_z:.6})"));
    }

    if violations.is_empty() {
        None
    } else {
        Some(violations)
    }
}

fn dump_trace(
    ctx: &mut DispatchContext<'_>,
    world: &crate::world::WorldState,
    path: &PathBuf,
) -> Result<()> {
    use anyhow::Context;

    let mut trace_lines = Vec::new();
    trace_lines.push(format!(
        "map={}x{} pawns={}",
        world.width(),
        world.height(),
        world.pawns().len()
    ));

    for pawn in world.pawns() {
        let body_def = pawn
            .body
            .as_deref()
            .and_then(|name| ctx.defs.body_type_defs.get(name));
        let head_def = pawn
            .head
            .as_deref()
            .and_then(|name| ctx.defs.head_type_defs.get(name));
        let hair_def = pawn
            .hair
            .as_deref()
            .and_then(|name| ctx.defs.hair_defs.get(name));
        let beard_def = pawn
            .beard
            .as_deref()
            .and_then(|name| ctx.defs.beard_defs.get(name));

        trace_lines.push(format!(
            "pawn={} facing={:?} body={} head={:?} hair={:?} beard={:?} apparel_count={}",
            pawn.label,
            pawn.facing,
            body_def
                .map(|b| b.body_naked_graphic_path.as_str())
                .unwrap_or("<none>"),
            head_def.map(|h| h.graphic_path.as_str()),
            hair_def.map(|h| h.tex_path.as_str()),
            beard_def.map(|b| b.tex_path.as_str()),
            pawn.apparel_defs.len()
        ));
    }

    std::fs::write(path, trace_lines.join("\n"))
        .with_context(|| format!("writing pawn trace to {}", path.display()))?;
    info!("wrote pawn trace: {}", path.display());
    Ok(())
}
