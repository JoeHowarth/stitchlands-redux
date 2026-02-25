use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};

use crate::defs::ThingDef;

#[derive(Debug, Clone)]
pub struct SpriteAsset {
    pub image: RgbaImage,
    pub source_path: Option<PathBuf>,
    pub used_fallback: bool,
}

pub fn resolve_sprite(core_data_dir: &Path, thing_def: &ThingDef) -> Result<SpriteAsset> {
    let textures_dir = core_data_dir.join("Core").join("Textures");
    if !textures_dir.exists() {
        return Ok(SpriteAsset {
            image: checkerboard_image(64, 64),
            source_path: None,
            used_fallback: true,
        });
    }

    let tex_path = thing_def.graphic_data.tex_path.as_str();
    let candidates = [
        textures_dir.join(format!("{}.png", tex_path)),
        textures_dir.join(format!("{}_south.png", tex_path)),
        textures_dir.join(format!("{}_north.png", tex_path)),
    ];

    for candidate in candidates {
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
        });
    }

    Ok(SpriteAsset {
        image: checkerboard_image(64, 64),
        source_path: None,
        used_fallback: true,
    })
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
    use super::*;

    #[test]
    fn creates_checkerboard() {
        let img = checkerboard_image(16, 16);
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
        assert_ne!(img.get_pixel(0, 0), img.get_pixel(8, 0));
    }
}
