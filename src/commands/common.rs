use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::assets::AssetResolver;
use crate::assets::extract_all_packed_textures;
use crate::defs::{
    ApparelDef, ApparelSkipFlagDef, BeardDefRender, BodyTypeDefRender, HairDefRender,
    HeadTypeDefRender, TerrainDef, ThingDef,
};
use crate::pawn::PawnFacing;

pub struct DefSet<'a> {
    pub thing_defs: &'a HashMap<String, ThingDef>,
    pub terrain_defs: &'a HashMap<String, TerrainDef>,
    pub apparel_defs: &'a HashMap<String, ApparelDef>,
    pub body_type_defs: &'a HashMap<String, BodyTypeDefRender>,
    pub head_type_defs: &'a HashMap<String, HeadTypeDefRender>,
    pub beard_defs: &'a HashMap<String, BeardDefRender>,
    pub hair_defs: &'a HashMap<String, HairDefRender>,
}

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

// --- Shared pawn/apparel helpers (moved from v1_scene.rs) ---

pub(crate) struct DirectionalTexturePath {
    pub path: String,
    pub data_facing: PawnFacing,
}

pub(crate) fn resolve_directional_tex_path(
    asset_resolver: &mut AssetResolver,
    data_dir: &Path,
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
        PawnFacing::East => &[
            (PawnFacing::East, "_east"),
            (PawnFacing::West, "_west"),
        ],
        PawnFacing::West => &[
            (PawnFacing::West, "_west"),
            (PawnFacing::East, "_east"),
        ],
    };

    for (data_facing, suffix) in candidates {
        let candidate = format!("{path}{suffix}");
        if let Ok(resolved) = asset_resolver.resolve_texture_path(data_dir, &candidate)
            && !resolved.sprite.used_fallback
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

pub(crate) fn apparel_worn_data_for_facing(
    apparel: &ApparelDef,
    facing: PawnFacing,
    body_type: Option<&str>,
) -> crate::defs::ApparelWornDirectionDef {
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

pub(crate) fn apparel_draw_layer_for_facing(
    apparel: &ApparelDef,
    facing: PawnFacing,
) -> Option<f32> {
    match facing {
        PawnFacing::North => apparel.draw_data.north_layer,
        PawnFacing::East => apparel.draw_data.east_layer,
        PawnFacing::South => apparel.draw_data.south_layer,
        PawnFacing::West => apparel.draw_data.west_layer,
    }
}

pub(crate) fn map_explicit_skip_flags(
    flags: &Option<Vec<ApparelSkipFlagDef>>,
) -> (bool, bool, bool) {
    let Some(flags) = flags else {
        return (false, false, false);
    };
    let mut skip_hair = false;
    let mut skip_beard = false;
    for flag in flags {
        match flag {
            ApparelSkipFlagDef::Hair => skip_hair = true,
            ApparelSkipFlagDef::Beard => skip_beard = true,
            ApparelSkipFlagDef::None | ApparelSkipFlagDef::Eyes => {}
        }
    }
    (skip_hair, skip_beard, true)
}

pub(crate) fn body_head_compatible(body_tex: &str, head_tex: &str) -> bool {
    let body_lower = body_tex.to_ascii_lowercase();
    let head_lower = head_tex.to_ascii_lowercase();
    if body_lower.contains("female") {
        return head_lower.contains("female");
    }
    if body_lower.contains("male") {
        return head_lower.contains("male");
    }
    true
}

pub(crate) fn make_missing_def_message(
    thingdef: &str,
    defs: &HashMap<String, ThingDef>,
) -> String {
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

pub(crate) fn build_apparel_tex_path(
    apparel: &ApparelDef,
    body_def_name: Option<&str>,
    render_as_pack: bool,
    asset_resolver: &mut AssetResolver,
    data_dir: &Path,
) -> String {
    use crate::pawn::ApparelLayer as ComposeApparelLayer;

    let layer: ComposeApparelLayer = apparel.layer.into();
    if matches!(
        layer,
        ComposeApparelLayer::OnSkin | ComposeApparelLayer::Middle | ComposeApparelLayer::Shell
    ) && !render_as_pack
        && let Some(body_name) = body_def_name
    {
        let suffixed = format!("{}_{}", apparel.tex_path, body_name);
        if let Ok(resolved) = asset_resolver.resolve_texture_path(data_dir, &suffixed)
            && !resolved.sprite.used_fallback
        {
            return suffixed;
        }
    }
    apparel.tex_path.clone()
}

pub(crate) fn build_full_apparel_layer_override(
    apparel: &ApparelDef,
    facing: PawnFacing,
    render_as_pack: bool,
) -> Option<f32> {
    apparel_draw_layer_for_facing(apparel, facing).or_else(|| {
        if apparel.layer == crate::defs::ApparelLayerDef::Shell
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

pub(crate) fn select_fixture_apparel_names(
    pawn_index: usize,
    pawn_fixture_variant: usize,
    pawn_focus_only: bool,
    body_layer_apparel: &[&ApparelDef],
    shell_layer_apparel: &[&ApparelDef],
    head_layer_apparel: &[&ApparelDef],
) -> Vec<String> {
    let seed = pawn_fixture_variant + pawn_index * 17;
    let mut out = Vec::new();

    if !body_layer_apparel.is_empty() && (pawn_focus_only || !pawn_index.is_multiple_of(4)) {
        let idx = seed % body_layer_apparel.len();
        out.push(body_layer_apparel[idx].def_name.clone());
    }
    if !shell_layer_apparel.is_empty() && (pawn_focus_only || pawn_index.is_multiple_of(2)) {
        let idx = (seed / 2).max(1) % shell_layer_apparel.len();
        out.push(shell_layer_apparel[idx].def_name.clone());
    }
    if !head_layer_apparel.is_empty() && (pawn_focus_only || !pawn_index.is_multiple_of(3)) {
        let idx = (seed / 3).max(1) % head_layer_apparel.len();
        out.push(head_layer_apparel[idx].def_name.clone());
    }

    if out.is_empty() {
        if !shell_layer_apparel.is_empty() {
            out.push(shell_layer_apparel[seed % shell_layer_apparel.len()].def_name.clone());
        } else if !body_layer_apparel.is_empty() {
            out.push(body_layer_apparel[seed % body_layer_apparel.len()].def_name.clone());
        } else if !head_layer_apparel.is_empty() {
            out.push(head_layer_apparel[seed % head_layer_apparel.len()].def_name.clone());
        }
    }

    out
}
