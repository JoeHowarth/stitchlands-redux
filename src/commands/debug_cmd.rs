use anyhow::Result;
use log::{info, warn};

use crate::cli::DebugCmd;

use super::{CommandAction, DispatchContext};

pub fn run(ctx: &mut DispatchContext<'_>, command: DebugCmd) -> Result<CommandAction> {
    match command {
        DebugCmd::ExtractPackedTextures { output_dir } => {
            super::common::run_extract_packed_textures(
                ctx.asset_resolver.packed_roots(),
                ctx.asset_resolver.typetree_registries(),
                &output_dir,
            )?;
            info!(
                "extract command complete for output dir {}",
                output_dir.display()
            );
            Ok(CommandAction::Done)
        }
        DebugCmd::SearchPackedTextures {
            query,
            search_limit,
        } => {
            super::common::print_packed_texture_search(ctx.asset_resolver, &query, search_limit);
            Ok(CommandAction::Done)
        }
        DebugCmd::DiagnoseTextures => {
            super::common::diagnose_textures(
                ctx.data_dir,
                ctx.asset_resolver.texture_roots(),
                ctx.asset_resolver.packed_roots(),
            );
            Ok(CommandAction::Done)
        }
        DebugCmd::ListDefs {
            def_filter,
            list_limit,
        } => {
            super::common::list_defs(ctx.defs.thing_defs, def_filter.as_deref(), list_limit);
            Ok(CommandAction::Done)
        }
        DebugCmd::ProbeTerrain {
            terrain_probe_limit,
        } => {
            super::common::run_terrain_probe(
                ctx.defs.terrain_defs,
                ctx.asset_resolver,
                terrain_probe_limit,
            )?;
            Ok(CommandAction::Done)
        }
        DebugCmd::ProbeDefs => {
            super::common::run_defs_probe(&ctx.defs, ctx.asset_resolver)?;
            Ok(CommandAction::Done)
        }
        DebugCmd::PackedDecodeProbe {
            sample_limit,
            min_attempts,
        } => {
            if let Some(outcome) = ctx
                .asset_resolver
                .run_packed_decode_probe(sample_limit, min_attempts)?
            {
                info!(
                    "packed decode probe: attempted={} succeeded={}",
                    outcome.attempted, outcome.succeeded
                );
                if outcome.disable_packed {
                    warn!(
                        "packed decode probe found 0 successful decodes in {} samples; disabling packed decode for this run",
                        outcome.attempted
                    );
                    for sample in outcome.sample_errors {
                        info!("packed decode probe sample failure: {sample}");
                    }
                }
            }
            Ok(CommandAction::Done)
        }
        DebugCmd::SearchPackedContainer {
            query,
            search_limit,
        } => {
            match ctx
                .asset_resolver
                .search_packed_container(&query, search_limit)?
            {
                Some(paths) => {
                    info!("{} container paths match '{query}':", paths.len());
                    for path in paths {
                        info!("  {path}");
                    }
                }
                None => warn!("search-packed-container: no packed roots loaded"),
            }
            Ok(CommandAction::Done)
        }
        DebugCmd::ProbeFolderVariant {
            tex_path,
            variant_index,
        } => {
            match ctx
                .asset_resolver
                .probe_folder_variant(&tex_path, variant_index)?
            {
                Some(label) => info!("folder variant {variant_index} of '{tex_path}' -> {label}"),
                None => warn!(
                    "folder variant {variant_index} of '{tex_path}' -> no match (folder empty or packed disabled)"
                ),
            }
            Ok(CommandAction::Done)
        }
        DebugCmd::PackedClassProbe { sample_limit } => {
            let ran = ctx.asset_resolver.run_packed_class_probe(sample_limit)?;
            if !ran {
                warn!("packed class probe: no packed roots loaded");
            }
            Ok(CommandAction::Done)
        }
        DebugCmd::ValidateFixture { path } => {
            let fixture = crate::fixtures::load_fixture(&path)?;
            info!(
                "fixture valid: {} schema={} map={}x{} terrain={} things={} pawns={}",
                path.display(),
                fixture.schema_version,
                fixture.map.width,
                fixture.map.height,
                fixture.map.terrain.len(),
                fixture.things.len(),
                fixture.pawns.len()
            );
            Ok(CommandAction::Done)
        }
    }
}
