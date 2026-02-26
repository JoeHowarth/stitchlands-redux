use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::assets::AssetResolver;
use crate::assets::extract_all_packed_textures;
use crate::defs::{TerrainDef, ThingDef};

pub fn run_extract_packed_textures(
    packed_roots: &[PathBuf],
    typetree_registries: &[PathBuf],
    output_dir: &Path,
) -> Result<()> {
    let summary = extract_all_packed_textures(packed_roots, typetree_registries, output_dir)
        .with_context(|| format!("extracting packed textures into {}", output_dir.display()))?;
    log::info!(
        "packed texture extraction finished: scanned={} exported={} failed={}",
        summary.scanned_textures,
        summary.exported_textures,
        summary.failed_textures
    );
    Ok(())
}

pub fn print_packed_texture_search(resolver: &AssetResolver, query: &str, limit: usize) {
    if let Some(matches) = resolver.search_packed_names(query, limit) {
        for name in &matches {
            println!("{name}");
        }
        println!("matched {} packed texture names", matches.len());
    } else {
        println!("no packed texture index loaded");
    }
}

pub fn diagnose_textures(data_dir: &Path, texture_roots: &[PathBuf], packed_roots: &[PathBuf]) {
    let roots = [
        data_dir.join("Core").join("Textures"),
        data_dir.join("Textures"),
    ];

    for root in roots {
        if !root.exists() {
            println!("missing: {}", root.display());
            continue;
        }
        let png_count = WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .count();
        println!("root: {} | png files: {}", root.display(), png_count);
    }

    for extra in texture_roots {
        let png_count = WalkDir::new(extra)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .count();
        println!("extra root: {} | png files: {}", extra.display(), png_count);
    }

    for root in packed_roots {
        println!(
            "packed candidate: {} | exists={}",
            root.display(),
            root.exists()
        );
    }

    println!(
        "tip: if counts are near zero, this install likely stores textures in Unity assets; keep using fallback or point --texture-root to an extracted texture dump"
    );
}

pub fn list_defs(
    defs: &std::collections::HashMap<String, ThingDef>,
    filter: Option<&str>,
    limit: usize,
) {
    let filter_lower = filter.map(|f| f.to_lowercase());
    let mut rows: Vec<_> = defs.values().collect();
    rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));

    let mut shown = 0usize;
    for thing in rows {
        if shown >= limit {
            break;
        }
        if let Some(f) = &filter_lower {
            let name = thing.def_name.to_lowercase();
            let tex = thing.graphic_data.tex_path.to_lowercase();
            if !name.contains(f) && !tex.contains(f) {
                continue;
            }
        }
        println!(
            "{} | texPath={} | class={}",
            thing.def_name,
            thing.graphic_data.tex_path,
            thing
                .graphic_data
                .graphic_class
                .as_deref()
                .unwrap_or("Graphic_Single")
        );
        shown += 1;
    }

    println!("shown {shown} defs (limit {limit})");
}

pub fn run_terrain_probe(
    data_dir: &Path,
    terrain_defs: &std::collections::HashMap<String, TerrainDef>,
    resolver: &mut AssetResolver,
    limit: usize,
) -> Result<()> {
    let mut rows: Vec<_> = terrain_defs.values().collect();
    rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut success = 0usize;
    let mut failed = 0usize;

    for terrain in rows.into_iter().take(limit) {
        let resolved = resolver.resolve_texture_path(data_dir, terrain.texture_path.as_str())?;
        if resolved.sprite.used_fallback {
            failed += 1;
            println!(
                "FAIL {:<28} texPath={} source=<fallback>",
                terrain.def_name, terrain.texture_path
            );
        } else {
            success += 1;
            let source = resolved
                .sprite
                .source_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            println!(
                "OK   {:<28} texPath={} source={}",
                terrain.def_name, terrain.texture_path, source
            );
        }
    }

    println!(
        "terrain probe summary: checked={} ok={} failed={}",
        limit, success, failed
    );
    Ok(())
}
