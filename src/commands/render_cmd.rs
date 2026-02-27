use anyhow::{Context, Result};
use glam::Vec3;
use log::{info, warn};

use crate::cli::RenderCmd;
use crate::renderer::SpriteParams;

use super::{CommandAction, DispatchContext, LaunchSpec};

pub fn run(ctx: &mut DispatchContext<'_>, render: RenderCmd) -> Result<CommandAction> {
    let (should_run_renderer, render_options, hide_window) =
        crate::cli::render_runtime(&render.view);
    if let Some(image_path) = &render.image_path {
        let image = image::open(image_path)
            .with_context(|| format!("loading image {}", image_path.display()))?
            .to_rgba8();
        info!("loaded direct image asset: {}", image_path.display());

        let sprite = crate::viewer::RenderSprite {
            def_name: format!("image:{}", image_path.display()),
            image,
            params: SpriteParams {
                world_pos: Vec3::new(render.cell_x + 0.5, render.cell_z + 0.5, 0.0),
                size: Vec3::new(render.scale, render.scale, 0.0).truncate(),
                tint: render.tint,
            },
            used_fallback: false,
            pawn_id: None,
        };

        if let Some(screenshot) = &render.view.screenshot {
            info!("screenshot output: {}", screenshot.display());
        }
        if !should_run_renderer {
            if let Some(export_path) = &render.export_resolved {
                sprite
                    .image
                    .save(export_path)
                    .with_context(|| format!("saving image to {}", export_path.display()))?;
                info!("wrote image export: {}", export_path.display());
            }
            return Ok(CommandAction::Done);
        }

        return Ok(CommandAction::Launch(Box::new(LaunchSpec {
            static_sprites: vec![sprite],
            dynamic_sprites: Vec::new(),
            runtime: None,
            runtime_tick_limit: None,
            screenshot: render.view.screenshot,
            camera_focus: None,
            render_options,
            hide_window,
            fixed_step: false,
        })));
    }

    let thingdef = render
        .thingdef
        .as_deref()
        .context("--thingdef or --image-path is required for render")?;
    let thing = ctx
        .thing_defs
        .get(thingdef)
        .cloned()
        .with_context(|| super::v1_scene::make_missing_def_message(thingdef, ctx.thing_defs))?;
    info!("selected def: {}", thing.def_name);

    let mut selected_defs = vec![thing];
    for extra_name in &render.extra_thingdef {
        let extra = ctx
            .thing_defs
            .get(extra_name)
            .cloned()
            .with_context(|| super::v1_scene::make_missing_def_message(extra_name, ctx.thing_defs))?;
        info!("selected extra def: {}", extra.def_name);
        selected_defs.push(extra);
    }

    let mut render_sprites = Vec::with_capacity(selected_defs.len());
    for (index, selected) in selected_defs.iter().enumerate() {
        let resolved = ctx
            .asset_resolver
            .resolve_thing(ctx.data_dir, selected)
            .with_context(|| {
                format!(
                    "resolving texture for def '{}' path '{}'",
                    selected.def_name, selected.graphic_data.tex_path
                )
            })?;

        if resolved.sprite.used_fallback {
            if ctx
                .asset_resolver
                .can_try_packed(&selected.graphic_data.tex_path)
                && let Some(probe) = ctx
                    .asset_resolver
                    .maybe_probe_decode_candidates(&selected.graphic_data.tex_path, 8)?
                && probe.attempted > 0
            {
                warn!(
                    "packed texture probe for '{}' attempted {} candidates, {} decodable",
                    selected.def_name, probe.attempted, probe.succeeded
                );
                for (name, err) in probe.sample_errors {
                    info!("packed candidate '{}' failed decode: {}", name, err);
                }
                if probe.succeeded == 0 {
                    warn!(
                        "no decodable packed candidates for '{}'; this usually means stripped/missing TypeTree metadata for this Unity build",
                        selected.def_name
                    );
                }
            }
            if !ctx.allow_fallback {
                anyhow::bail!(
                    "texture missing for '{}' ({}) and fallback is disabled",
                    selected.def_name,
                    selected.graphic_data.tex_path
                );
            }
            warn!(
                "texture missing for '{}' ({}) - using checker fallback",
                selected.def_name, selected.graphic_data.tex_path
            );
            for attempted in resolved.sprite.attempted_paths.iter().take(6) {
                info!("attempted: {}", attempted.display());
            }
        }

        if let Some(path) = &resolved.sprite.source_path {
            if resolved.resolved_from_packed {
                info!("resolved texture (packed): {}", path.display());
            } else if resolved.sprite.resolved_with_fuzzy_match {
                info!("resolved texture (fuzzy): {}", path.display());
            } else {
                info!("resolved texture: {}", path.display());
            }
        }

        if let Some(export_path) = &render.export_resolved {
            let with_suffix = if selected_defs.len() == 1 {
                export_path.clone()
            } else {
                let stem = export_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("resolved");
                let ext = export_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let filename = format!("{stem}_{}_{}.{}", index, selected.def_name, ext);
                export_path.with_file_name(filename)
            };
            resolved
                .sprite
                .image
                .save(&with_suffix)
                .with_context(|| format!("saving resolved sprite to {}", with_suffix.display()))?;
            info!("wrote resolved sprite image: {}", with_suffix.display());
        }

        let size = selected.graphic_data.draw_size * render.scale;
        let draw_offset = selected.graphic_data.draw_offset;
        let (grid_x, grid_y) = if render.sheet_columns > 0 {
            (
                (index % render.sheet_columns) as f32,
                (index / render.sheet_columns) as f32,
            )
        } else {
            (index as f32, 0.0)
        };
        let world_pos = Vec3::new(
            render.cell_x + (grid_x * render.sheet_spacing) + 0.5 + draw_offset.x,
            render.cell_z - (grid_y * render.sheet_spacing) + 0.5 + draw_offset.z,
            draw_offset.y,
        );
        let tint = [
            render.tint[0] * selected.graphic_data.color.r,
            render.tint[1] * selected.graphic_data.color.g,
            render.tint[2] * selected.graphic_data.color.b,
            render.tint[3] * selected.graphic_data.color.a,
        ];

        info!(
            "sprite params [{}] {} -> size=({:.2}, {:.2}) offset=({:.2}, {:.2}, {:.2})",
            index, selected.def_name, size.x, size.y, draw_offset.x, draw_offset.y, draw_offset.z
        );

        render_sprites.push(crate::viewer::RenderSprite {
            def_name: selected.def_name.clone(),
            image: resolved.sprite.image,
            params: SpriteParams {
                world_pos,
                size,
                tint,
            },
            used_fallback: resolved.sprite.used_fallback,
            pawn_id: None,
        });
    }

    if !should_run_renderer {
        return Ok(CommandAction::Done);
    }

    Ok(CommandAction::Launch(Box::new(LaunchSpec {
        static_sprites: render_sprites,
        dynamic_sprites: Vec::new(),
        runtime: None,
        runtime_tick_limit: None,
        screenshot: render.view.screenshot,
        camera_focus: None,
        render_options,
        hide_window,
        fixed_step: false,
    })))
}
