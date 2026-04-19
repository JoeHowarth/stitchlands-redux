use std::path::{Path, PathBuf};

use crate::defs::GraphicKind;

pub struct TextureVariants<'a> {
    pub tex_path: &'a str,
    pub kind: GraphicKind,
}

pub fn variants_for(tex_path: &str, kind: GraphicKind) -> TextureVariants<'_> {
    TextureVariants { tex_path, kind }
}

impl<'a> TextureVariants<'a> {
    pub fn base_names(&self) -> Vec<String> {
        let basename = self
            .tex_path
            .rsplit('/')
            .next()
            .unwrap_or(self.tex_path)
            .to_ascii_lowercase();
        vec![
            basename.clone(),
            format!("{basename}_south"),
            format!("{basename}_north"),
            format!("{basename}_east"),
            format!("{basename}_west"),
        ]
    }

    pub fn disk_candidates(&self, root: &Path) -> Vec<PathBuf> {
        vec![
            root.join(format!("{}.png", self.tex_path)),
            root.join(format!("{}_south.png", self.tex_path)),
            root.join(format!("{}_north.png", self.tex_path)),
        ]
    }

    pub fn container_paths(&self) -> Vec<String> {
        let prefixed = self.prefixed_lower();
        vec![
            prefixed.clone(),
            format!("{prefixed}.png"),
            format!("{prefixed}_south.png"),
            format!("{prefixed}_north.png"),
            format!("{prefixed}_east.png"),
            format!("{prefixed}_west.png"),
        ]
    }

    pub fn folder_prefix(&self) -> Option<String> {
        self.folder_key().map(|key| format!("{key}/"))
    }

    pub fn folder_key(&self) -> Option<String> {
        if self.kind.is_random() {
            Some(self.prefixed_lower())
        } else {
            None
        }
    }

    fn prefixed_lower(&self) -> String {
        let base = self.tex_path.to_ascii_lowercase();
        if base.starts_with("textures/") {
            base
        } else {
            format!("textures/{base}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_names_include_directional_suffixes() {
        let v = variants_for("Things/Item/Resource/Steel", GraphicKind::Single);
        let names = v.base_names();
        assert_eq!(names[0], "steel");
        assert!(names.contains(&"steel_south".to_string()));
        assert!(names.contains(&"steel_east".to_string()));
    }

    #[test]
    fn container_paths_are_prefixed_exact_paths() {
        let v = variants_for("Things/Item/Resource/Steel", GraphicKind::Single);
        let paths = v.container_paths();
        assert!(paths.contains(&"textures/things/item/resource/steel.png".to_string()));
        assert!(paths.contains(&"textures/things/item/resource/steel_south.png".to_string()));
    }

    #[test]
    fn folder_prefix_only_for_random() {
        let s = variants_for("Things/Item/Chunk/ChunkSlag", GraphicKind::Single);
        assert_eq!(s.folder_prefix(), None);
        assert_eq!(s.folder_key(), None);
        let r = variants_for("Things/Item/Chunk/ChunkSlag", GraphicKind::Random);
        assert_eq!(
            r.folder_prefix().as_deref(),
            Some("textures/things/item/chunk/chunkslag/")
        );
        assert_eq!(
            r.folder_key().as_deref(),
            Some("textures/things/item/chunk/chunkslag")
        );
    }
}
