use std::path::PathBuf;

use anyhow::Result;
use image::RgbaImage;

use crate::defs::{GraphicKind, ThingDef};

#[derive(Debug, Clone, Copy)]
pub struct TextureQuery<'a> {
    pub tex_path: &'a str,
    pub kind: GraphicKind,
    pub variant_index: usize,
}

impl<'a> TextureQuery<'a> {
    pub fn single(tex_path: &'a str) -> Self {
        Self {
            tex_path,
            kind: GraphicKind::Single,
            variant_index: 0,
        }
    }

    pub fn for_thing(thing_def: &'a ThingDef, variant_index: usize) -> Self {
        Self {
            tex_path: thing_def.graphic_data.tex_path.as_str(),
            kind: thing_def.graphic_data.kind,
            variant_index,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SpriteSource {
    Disk(PathBuf),
    Packed { label: String },
    Fallback { attempted: Vec<PathBuf> },
}

pub struct ResolvedSprite {
    pub image: RgbaImage,
    pub source: SpriteSource,
}

impl ResolvedSprite {
    pub fn used_fallback(&self) -> bool {
        matches!(self.source, SpriteSource::Fallback { .. })
    }

    pub fn resolved_from_packed(&self) -> bool {
        matches!(self.source, SpriteSource::Packed { .. })
    }

    pub fn source_path(&self) -> Option<PathBuf> {
        match &self.source {
            SpriteSource::Disk(p) => Some(p.clone()),
            SpriteSource::Packed { label } => Some(PathBuf::from(label)),
            SpriteSource::Fallback { .. } => None,
        }
    }

    pub fn attempted_paths(&self) -> &[PathBuf] {
        match &self.source {
            SpriteSource::Fallback { attempted } => attempted,
            _ => &[],
        }
    }
}

pub enum BackendLookup {
    Hit(ResolvedSprite),
    Miss { attempted: Vec<PathBuf> },
}

pub trait TextureBackend {
    fn lookup(&mut self, query: &TextureQuery) -> Result<BackendLookup>;
}
