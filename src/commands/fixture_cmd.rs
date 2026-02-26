use anyhow::Result;

use crate::cli::{AuditCmd, FixtureCmd};

use super::{CommandAction, DispatchContext, LaunchSpec};

pub fn run_fixture(ctx: &mut DispatchContext<'_>, mode: FixtureCmd) -> Result<CommandAction> {
    if let FixtureCmd::V2(args) = mode {
        return super::fixture_v2_cmd::run_fixture_v2(ctx, args);
    }

    let (fixture, is_pawn) = match mode {
        FixtureCmd::V1(args) => (args, false),
        FixtureCmd::Pawn(args) => (args, true),
        FixtureCmd::V2(_) => unreachable!("v2 handled above"),
    };
    let (should_run_renderer, render_options, hide_window) =
        crate::cli::render_runtime(&fixture.view);
    let (render_sprites, camera_focus) = if !is_pawn {
        crate::build_v1_fixture_scene(crate::FixtureSceneConfig {
            data_dir: ctx.data_dir,
            thing_defs: ctx.thing_defs,
            terrain_defs: ctx.terrain_defs,
            apparel_defs: ctx.apparel_defs,
            body_type_defs: ctx.body_type_defs,
            head_type_defs: ctx.head_type_defs,
            beard_defs: ctx.beard_defs,
            hair_defs: ctx.hair_defs,
            asset_resolver: ctx.asset_resolver,
            width: fixture.map_width,
            height: fixture.map_height,
            pawn_focus_only: false,
            pawn_audit_mode: false,
            pawn_count: 6,
            pawn_fixture_variant: fixture.pawn_fixture_variant,
            dump_pawn_trace: fixture.dump_pawn_trace.clone(),
            compose_config: ctx.compose_config.clone(),
            strict_missing: !ctx.allow_fallback,
        })?
    } else {
        crate::build_v1_fixture_scene(crate::FixtureSceneConfig {
            data_dir: ctx.data_dir,
            thing_defs: ctx.thing_defs,
            terrain_defs: ctx.terrain_defs,
            apparel_defs: ctx.apparel_defs,
            body_type_defs: ctx.body_type_defs,
            head_type_defs: ctx.head_type_defs,
            beard_defs: ctx.beard_defs,
            hair_defs: ctx.hair_defs,
            asset_resolver: ctx.asset_resolver,
            width: fixture.map_width.clamp(8, 18),
            height: fixture.map_height.clamp(8, 18),
            pawn_focus_only: true,
            pawn_audit_mode: false,
            pawn_count: 1,
            pawn_fixture_variant: fixture.pawn_fixture_variant,
            dump_pawn_trace: fixture.dump_pawn_trace.clone(),
            compose_config: ctx.compose_config.clone(),
            strict_missing: !ctx.allow_fallback,
        })?
    };

    if !should_run_renderer {
        return Ok(CommandAction::Done);
    }
    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites: render_sprites,
        dynamic_sprites: Vec::new(),
        runtime: None,
        screenshot: fixture.view.screenshot,
        camera_focus: Some(camera_focus),
        render_options,
        hide_window,
        fixed_step: false,
    })))
}

pub fn run_audit(ctx: &mut DispatchContext<'_>, audit: AuditCmd) -> Result<CommandAction> {
    let (should_run_renderer, render_options, hide_window) =
        crate::cli::render_runtime(&audit.view);
    let (render_sprites, camera_focus) =
        crate::build_v1_fixture_scene(crate::FixtureSceneConfig {
            data_dir: ctx.data_dir,
            thing_defs: ctx.thing_defs,
            terrain_defs: ctx.terrain_defs,
            apparel_defs: ctx.apparel_defs,
            body_type_defs: ctx.body_type_defs,
            head_type_defs: ctx.head_type_defs,
            beard_defs: ctx.beard_defs,
            hair_defs: ctx.hair_defs,
            asset_resolver: ctx.asset_resolver,
            width: audit.map_width.max(24),
            height: audit.map_height.max(24),
            pawn_focus_only: false,
            pawn_audit_mode: true,
            pawn_count: audit.pawn_count.clamp(6, 20),
            pawn_fixture_variant: audit.pawn_fixture_variant,
            dump_pawn_trace: audit.dump_pawn_trace.clone(),
            compose_config: ctx.compose_config.clone(),
            strict_missing: !ctx.allow_fallback,
        })?;

    if !should_run_renderer {
        return Ok(CommandAction::Done);
    }
    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites: render_sprites,
        dynamic_sprites: Vec::new(),
        runtime: None,
        screenshot: audit.view.screenshot,
        camera_focus: Some(camera_focus),
        render_options,
        hide_window,
        fixed_step: false,
    })))
}
