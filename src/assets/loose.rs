use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};

use crate::assets::variants::variants_for;
use crate::defs::{GraphicKind, ThingDef};

#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub image: RgbaImage,
    pub source_path: Option<PathBuf>,
    pub used_fallback: bool,
    pub attempted_paths: Vec<PathBuf>,
}

pub fn resolve_sprite(
    core_data_dir: &Path,
    thing_def: &ThingDef,
    extra_texture_roots: &[PathBuf],
) -> Result<SpriteAsset> {
    resolve_texture_path(
        core_data_dir,
        thing_def.graphic_data.tex_path.as_str(),
        extra_texture_roots,
    )
}

pub fn resolve_texture_path(
    core_data_dir: &Path,
    tex_path: &str,
    extra_texture_roots: &[PathBuf],
) -> Result<SpriteAsset> {
    let texture_roots = texture_roots(core_data_dir, extra_texture_roots);

    let mut attempted_paths = Vec::new();

    let variants = variants_for(tex_path, GraphicKind::Single);
    for root in &texture_roots {
        for candidate in variants.disk_candidates(root) {
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
            });
        }
    }

    Ok(SpriteAsset {
        image: checkerboard_image(64, 64),
        source_path: None,
        used_fallback: true,
        attempted_paths,
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
                kind: GraphicKind::Single,
                color: RgbaColor::WHITE,
                draw_size: Vec2::new(1.0, 1.0),
                draw_offset: Vec3::ZERO,
            },
        }
    }

}
