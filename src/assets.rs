use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};
use walkdir::WalkDir;

use crate::defs::ThingDef;

#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub image: RgbaImage,
    pub source_path: Option<PathBuf>,
    pub used_fallback: bool,
    pub attempted_paths: Vec<PathBuf>,
    pub resolved_with_fuzzy_match: bool,
}

pub fn resolve_sprite(
    core_data_dir: &Path,
    thing_def: &ThingDef,
    extra_texture_roots: &[PathBuf],
) -> Result<SpriteAsset> {
    let texture_roots = texture_roots(core_data_dir, extra_texture_roots);

    let tex_path = thing_def.graphic_data.tex_path.as_str();
    let mut attempted_paths = Vec::new();

    for root in &texture_roots {
        let candidates = [
            root.join(format!("{}.png", tex_path)),
            root.join(format!("{}_south.png", tex_path)),
            root.join(format!("{}_north.png", tex_path)),
        ];

        for candidate in candidates {
            attempted_paths.push(candidate.clone());
            if !candidate.exists() {
                continue;
            }

            let image = image::open(&candidate)
                .with_context(|| format!("failed to decode image {}", candidate.display()))?
                .to_rgba8();
            return Ok(SpriteAsset {
                image,
                source_path: Some(candidate),
                used_fallback: false,
                attempted_paths,
                resolved_with_fuzzy_match: false,
            });
        }
    }

    if let Some(fuzzy_path) = find_fuzzy_match(&texture_roots, tex_path) {
        let image = image::open(&fuzzy_path)
            .with_context(|| format!("failed to decode image {}", fuzzy_path.display()))?
            .to_rgba8();
        return Ok(SpriteAsset {
            image,
            source_path: Some(fuzzy_path),
            used_fallback: false,
            attempted_paths,
            resolved_with_fuzzy_match: true,
        });
    }

    Ok(SpriteAsset {
        image: checkerboard_image(64, 64),
        source_path: None,
        used_fallback: true,
        attempted_paths,
        resolved_with_fuzzy_match: false,
    })
}

fn texture_roots(core_data_dir: &Path, extra_texture_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = vec![
        core_data_dir.join("Core").join("Textures"),
        core_data_dir.join("Textures"),
    ];
    for extra in extra_texture_roots {
        roots.push(extra.to_path_buf());
    }
    roots
}

fn find_fuzzy_match(texture_roots: &[PathBuf], tex_path: &str) -> Option<PathBuf> {
    let basename = tex_path.rsplit('/').next().unwrap_or(tex_path);
    let wanted = [
        basename.to_ascii_lowercase(),
        format!("{}_south", basename.to_ascii_lowercase()),
        format!("{}_north", basename.to_ascii_lowercase()),
    ];

    for root in texture_roots {
        if !root.exists() {
            continue;
        }

        let mut best: Option<(i32, PathBuf)> = None;
        for entry in WalkDir::new(root)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("png"))
                .unwrap_or(false);
            if !ext {
                continue;
            }

            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_ascii_lowercase(),
                None => continue,
            };

            if !wanted.iter().any(|name| name == &stem) {
                continue;
            }

            let score = fuzzy_score(path, tex_path);
            match &best {
                Some((best_score, _)) if *best_score >= score => {}
                _ => best = Some((score, path.to_path_buf())),
            }
        }

        if let Some((_, path)) = best {
            return Some(path);
        }
    }

    None
}

fn fuzzy_score(path: &Path, tex_path: &str) -> i32 {
    let path_lower = path.to_string_lossy().to_ascii_lowercase();
    let tex_lower = tex_path.to_ascii_lowercase();
    let basename = tex_lower.rsplit('/').next().unwrap_or("unknown");

    let mut score = 0;
    if path_lower.contains(&tex_lower) {
        score += 100;
    }
    if path_lower.contains("/things/") {
        score += 25;
    }
    if path_lower.contains("/item/") {
        score += 10;
    }
    if path_lower.contains(basename) {
        score += 15;
    }
    score - path_lower.len() as i32 / 40
}

fn checkerboard_image(width: u32, height: u32) -> RgbaImage {
    let mut img: RgbaImage = ImageBuffer::new(width, height);
    let tile = 8;
    for y in 0..height {
        for x in 0..width {
            let is_dark = ((x / tile) + (y / tile)) % 2 == 0;
            let color = if is_dark {
                Rgba([180, 20, 20, 255])
            } else {
                Rgba([30, 30, 30, 255])
            };
            img.put_pixel(x, y, color);
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use std::fs;

    use glam::{Vec2, Vec3};

    use super::*;
    use crate::defs::{GraphicData, RgbaColor, ThingDef};

    #[test]
    fn creates_checkerboard() {
        let img = checkerboard_image(16, 16);
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
        assert_ne!(img.get_pixel(0, 0), img.get_pixel(8, 0));
    }

    #[test]
    fn resolves_exact_texture_path() {
        let root = std::env::temp_dir().join(format!("stitchlands-assets-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let tex_file = root
            .join("Core")
            .join("Textures")
            .join("Things")
            .join("Item")
            .join("Resource")
            .join("Steel.png");
        fs::create_dir_all(tex_file.parent().unwrap()).unwrap();
        checkerboard_image(4, 4).save(&tex_file).unwrap();

        let def = fake_thing("Things/Item/Resource/Steel");
        let sprite = resolve_sprite(&root, &def, &[]).unwrap();

        assert!(!sprite.used_fallback);
        assert_eq!(sprite.source_path.as_deref(), Some(tex_file.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_south_fallback() {
        let root =
            std::env::temp_dir().join(format!("stitchlands-assets-south-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let tex_file = root
            .join("Core")
            .join("Textures")
            .join("Things")
            .join("Item")
            .join("Resource")
            .join("Steel_south.png");
        fs::create_dir_all(tex_file.parent().unwrap()).unwrap();
        checkerboard_image(4, 4).save(&tex_file).unwrap();

        let def = fake_thing("Things/Item/Resource/Steel");
        let sprite = resolve_sprite(&root, &def, &[]).unwrap();

        assert!(!sprite.used_fallback);
        assert_eq!(sprite.source_path.as_deref(), Some(tex_file.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    fn fake_thing(tex_path: &str) -> ThingDef {
        ThingDef {
            def_name: "TestThing".to_string(),
            graphic_data: GraphicData {
                tex_path: tex_path.to_string(),
                graphic_class: None,
                shader_type: None,
                color: RgbaColor::WHITE,
                color_two: None,
                draw_size: Vec2::new(1.0, 1.0),
                draw_offset: Vec3::ZERO,
            },
        }
    }

    #[test]
    fn resolves_from_extra_root_using_fuzzy_basename() {
        let root =
            std::env::temp_dir().join(format!("stitchlands-assets-fuzzy-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let data_root = root.join("Data");
        fs::create_dir_all(data_root.join("Core/Defs")).unwrap();

        let extra = root.join("extracted");
        let tex_file = extra
            .join("dump")
            .join("textures")
            .join("resource")
            .join("Steel.png");
        fs::create_dir_all(tex_file.parent().unwrap()).unwrap();
        checkerboard_image(4, 4).save(&tex_file).unwrap();

        let def = fake_thing("Things/Item/Resource/Steel");
        let sprite = resolve_sprite(&data_root, &def, &[extra]).unwrap();

        assert!(!sprite.used_fallback);
        assert!(sprite.resolved_with_fuzzy_match);
        assert_eq!(sprite.source_path.as_deref(), Some(tex_file.as_path()));

        let _ = fs::remove_dir_all(root);
    }
}
