use std::path::PathBuf;

use anyhow::{Context, Result};
use image::{ImageBuffer, Rgba, RgbaImage};

use crate::assets::backend::{
    BackendLookup, ResolvedSprite, SpriteSource, TextureBackend, TextureQuery,
};
use crate::assets::variants::variants_for;

pub struct LooseFiles {
    core_data_dir: PathBuf,
    extra_texture_roots: Vec<PathBuf>,
}

impl LooseFiles {
    pub fn new(core_data_dir: PathBuf, extra_texture_roots: Vec<PathBuf>) -> Self {
        Self {
            core_data_dir,
            extra_texture_roots,
        }
    }

    pub fn extra_texture_roots(&self) -> &[PathBuf] {
        &self.extra_texture_roots
    }

    fn texture_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![
            self.core_data_dir.join("Core").join("Textures"),
            self.core_data_dir.join("Textures"),
        ];
        for extra in &self.extra_texture_roots {
            roots.push(extra.clone());
        }
        roots
    }
}

impl TextureBackend for LooseFiles {
    fn lookup(&mut self, query: &TextureQuery) -> Result<BackendLookup> {
        let roots = self.texture_roots();
        let variants = variants_for(query.tex_path, query.kind);
        for root in &roots {
            for candidate in variants.disk_candidates(root) {
                if !candidate.exists() {
                    continue;
                }
                let image = image::open(&candidate)
                    .with_context(|| format!("failed to decode image {}", candidate.display()))?
                    .to_rgba8();
                return Ok(BackendLookup::Hit(ResolvedSprite {
                    image,
                    source: SpriteSource::Disk(candidate),
                }));
            }
        }
        Ok(BackendLookup::Miss)
    }
}

pub fn checkerboard_image(width: u32, height: u32) -> RgbaImage {
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
    use crate::defs::{GraphicData, GraphicKind, RgbaColor, ThingDef};

    fn resolve_one(loose: &mut LooseFiles, tex_path: &str) -> (bool, Option<PathBuf>) {
        let query = TextureQuery::single(tex_path);
        match loose.lookup(&query).unwrap() {
            BackendLookup::Hit(sprite) => (false, sprite.source_path()),
            BackendLookup::Miss => (true, None),
        }
    }

    #[test]
    fn creates_checkerboard() {
        let img = checkerboard_image(16, 16);
        assert_eq!(img.width(), 16);
        assert_eq!(img.height(), 16);
        assert_ne!(img.get_pixel(0, 0), img.get_pixel(8, 0));
    }

    #[test]
    fn resolves_exact_texture_path() {
        let root = tmp_root("stitchlands-assets");
        let tex_file = root
            .join("Core")
            .join("Textures")
            .join("Things")
            .join("Item")
            .join("Resource")
            .join("Steel.png");
        fs::create_dir_all(tex_file.parent().unwrap()).unwrap();
        checkerboard_image(4, 4).save(&tex_file).unwrap();

        let mut loose = LooseFiles::new(root.clone(), Vec::new());
        let def = fake_thing("Things/Item/Resource/Steel");
        let query = TextureQuery::for_thing(&def, 0);
        let (missed, path) = match loose.lookup(&query).unwrap() {
            BackendLookup::Hit(sprite) => (false, sprite.source_path()),
            BackendLookup::Miss => (true, None),
        };
        assert!(!missed);
        assert_eq!(path.as_deref(), Some(tex_file.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_south_fallback() {
        let root = tmp_root("stitchlands-assets-south");
        let tex_file = root
            .join("Core")
            .join("Textures")
            .join("Things")
            .join("Item")
            .join("Resource")
            .join("Steel_south.png");
        fs::create_dir_all(tex_file.parent().unwrap()).unwrap();
        checkerboard_image(4, 4).save(&tex_file).unwrap();

        let mut loose = LooseFiles::new(root.clone(), Vec::new());
        let (missed, path) = resolve_one(&mut loose, "Things/Item/Resource/Steel");
        assert!(!missed);
        assert_eq!(path.as_deref(), Some(tex_file.as_path()));

        let _ = fs::remove_dir_all(root);
    }

    fn tmp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
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
                shadow_data: None,
                link_type: Default::default(),
                link_flags: Default::default(),
            },
            block_light: false,
            holds_roof: false,
            cast_edge_shadows: false,
            static_sun_shadow_height: 0.0,
            glower: None,
        }
    }
}
