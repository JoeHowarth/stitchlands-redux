use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::assets::AssetResolver;
use crate::assets::extract_all_packed_textures;
use crate::defs::{DefSet, TerrainDef, ThingDef};

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
    let matches = resolver.search_packed_names(query, limit);
    for name in &matches {
        println!("{name}");
    }
    println!("matched {} packed texture names", matches.len());
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
        "tip: if counts are near zero, this install likely stores textures in Unity assets; configure a TypeTree registry or point --texture-root to an extracted texture dump"
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
            "{} | texPath={} | class={:?}",
            thing.def_name, thing.graphic_data.tex_path, thing.graphic_data.kind
        );
        shown += 1;
    }

    println!("shown {shown} defs (limit {limit})");
}

pub fn run_defs_probe(defs: &DefSet<'_>, resolver: &mut AssetResolver) -> Result<()> {
    fn probe<T>(
        label: &str,
        resolver: &mut AssetResolver,
        defs: &HashMap<String, T>,
        name_of: impl Fn(&T) -> &str,
        tex_of: impl Fn(&T) -> Option<&str>,
    ) -> Result<()> {
        let mut rows: Vec<_> = defs.values().collect();
        rows.sort_by(|a, b| name_of(a).cmp(name_of(b)));
        let mut decoded = 0usize;
        let mut fallback = 0usize;
        let mut skipped = 0usize;
        for def in &rows {
            let Some(tex) = tex_of(def) else {
                skipped += 1;
                continue;
            };
            if tex.is_empty() {
                skipped += 1;
                continue;
            }
            let resolved = resolver.resolve_texture_path(tex)?;
            if resolved.used_fallback() {
                fallback += 1;
            } else {
                decoded += 1;
            }
        }
        let total = decoded + fallback;
        let skipped_note = if skipped > 0 {
            format!(" skipped={skipped}")
        } else {
            String::new()
        };
        println!("{label:<8} decoded={decoded}/{total} fallback={fallback}{skipped_note}");
        Ok(())
    }

    probe(
        "body",
        resolver,
        defs.body_type_defs,
        |d| d.def_name.as_str(),
        |d| Some(d.body_naked_graphic_path.as_str()),
    )?;
    probe(
        "head",
        resolver,
        defs.head_type_defs,
        |d| d.def_name.as_str(),
        |d| Some(d.graphic_path.as_str()),
    )?;
    probe(
        "hair",
        resolver,
        defs.hair_defs,
        |d| d.def_name.as_str(),
        |d| Some(d.tex_path.as_str()),
    )?;
    probe(
        "beard",
        resolver,
        defs.beard_defs,
        |d| d.def_name.as_str(),
        |d| {
            if d.no_graphic {
                None
            } else {
                Some(d.tex_path.as_str())
            }
        },
    )?;
    probe(
        "apparel",
        resolver,
        defs.apparel_defs,
        |d| d.def_name.as_str(),
        |d| Some(d.tex_path.as_str()),
    )?;
    Ok(())
}

pub fn run_thingdef_inheritance_audit(
    data_dir: &Path,
    thing_defs: &HashMap<String, ThingDef>,
    limit: usize,
) -> Result<()> {
    let audit = crate::defs::audit_thing_def_inheritance(data_dir, thing_defs)?;
    println!(
        "thingdef inheritance audit: raw_concrete={} loaded={} inherited_graphic_missing={}",
        audit.raw_concrete_defs,
        audit.loaded_defs,
        audit.missing_inherited_graphic_defs.len()
    );
    for def_name in audit.missing_inherited_graphic_defs.iter().take(limit) {
        println!("MISSING inherited graphicData: {def_name}");
    }
    if audit.missing_inherited_graphic_defs.len() > limit {
        println!(
            "... {} more",
            audit.missing_inherited_graphic_defs.len() - limit
        );
    }
    Ok(())
}

pub fn run_terrain_probe(
    terrain_defs: &std::collections::HashMap<String, TerrainDef>,
    resolver: &mut AssetResolver,
    limit: usize,
) -> Result<()> {
    let mut rows: Vec<_> = terrain_defs.values().collect();
    rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut success = 0usize;
    let mut failed = 0usize;

    for terrain in rows.into_iter().take(limit) {
        let resolved = resolver.resolve_texture_path(terrain.texture_path.as_str())?;
        if resolved.used_fallback() {
            failed += 1;
            println!(
                "FAIL {:<28} texPath={} source=<fallback>",
                terrain.def_name, terrain.texture_path
            );
        } else {
            success += 1;
            let source = resolved
                .source_path()
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

pub(crate) fn make_missing_def_message(thingdef: &str, defs: &HashMap<String, ThingDef>) -> String {
    let mut suggestions: Vec<&str> = defs
        .keys()
        .filter_map(|name| {
            if name.eq_ignore_ascii_case(thingdef) {
                Some(name.as_str())
            } else {
                let name_lower = name.to_lowercase();
                let query_lower = thingdef.to_lowercase();
                if name_lower.contains(&query_lower) || query_lower.contains(&name_lower) {
                    Some(name.as_str())
                } else {
                    None
                }
            }
        })
        .take(5)
        .collect();
    suggestions.sort_unstable();

    if suggestions.is_empty() {
        format!("thingdef '{thingdef}' not found")
    } else {
        format!(
            "thingdef '{thingdef}' not found. Close matches: {}",
            suggestions.join(", ")
        )
    }
}
