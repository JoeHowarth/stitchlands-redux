use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use log::info;
use unity_asset::environment::EnvironmentObjectRef;
use unity_asset_core::constants::class_ids;
use unity_asset_decode::texture::Texture2DConverter;
use unity_asset_decode::unity_version::UnityVersion;

pub struct PackedTextureIndex {
    signature: String,
    names: Vec<String>,
    container_paths: HashSet<String>,
}

impl PackedTextureIndex {
    pub fn load_or_build(
        roots: &[PathBuf],
        typetree_registries: &[PathBuf],
        cache_path: &Path,
        rebuild: bool,
    ) -> Result<Self> {
        let signature = roots_signature(roots, typetree_registries);
        if !rebuild
            && let Ok(cached) = Self::load(cache_path)
            && cached.signature == signature
        {
            info!(
                "loaded packed texture index cache: {} (names={} container={})",
                cache_path.display(),
                cached.names.len(),
                cached.container_paths.len()
            );
            return Ok(cached);
        }

        let built = Self::build(roots, typetree_registries, signature)?;
        built.save(cache_path)?;
        info!(
            "rebuilt packed texture index cache: {} (names={} container={})",
            cache_path.display(),
            built.names.len(),
            built.container_paths.len()
        );
        Ok(built)
    }

    pub fn search_names(&self, query: &str, limit: usize) -> Vec<String> {
        let needle = query.to_ascii_lowercase();
        let mut matches: Vec<String> = self
            .names
            .iter()
            .filter(|name| name.contains(&needle))
            .take(limit)
            .cloned()
            .collect();
        matches.sort();
        matches
    }

    pub fn maybe_contains(&self, tex_path: &str) -> bool {
        // Fail open if the index is effectively empty; this avoids false negatives
        // from stripped-name Unity layouts where metadata extraction is incomplete.
        if self.is_empty() {
            return true;
        }

        let basename = tex_path
            .rsplit('/')
            .next()
            .unwrap_or(tex_path)
            .to_ascii_lowercase();
        if self.has_prefix_match(&basename) {
            return true;
        }

        let variants =
            crate::assets::variants::variants_for(tex_path, crate::defs::GraphicKind::Single);
        for path in variants.container_paths() {
            if self.container_paths.contains(&path) {
                return true;
            }
        }

        false
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty() && self.container_paths.is_empty()
    }

    fn has_prefix_match(&self, basename: &str) -> bool {
        let start = self.names.partition_point(|name| name.as_str() < basename);
        let prefix_underscore = format!("{basename}_");
        for name in &self.names[start..] {
            if name == basename || name.starts_with(&prefix_underscore) {
                return true;
            }
            if !name.starts_with(basename) {
                break;
            }
        }
        false
    }

    fn build(
        roots: &[PathBuf],
        typetree_registries: &[PathBuf],
        signature: String,
    ) -> Result<Self> {
        let Some((env, _)) =
            crate::assets::packed_textures::build_packed_environment(roots, typetree_registries)?
        else {
            return Ok(Self {
                signature,
                names: Vec::new(),
                container_paths: HashSet::new(),
            });
        };

        let converter = Texture2DConverter::new(UnityVersion::default());
        let mut names_set = HashSet::new();
        for obj in env.objects() {
            let EnvironmentObjectRef::Binary(binary) = obj else {
                continue;
            };
            if binary.object.class_id() != class_ids::TEXTURE_2D {
                continue;
            }
            let key = binary.key();
            if let Ok(Some(name)) = env.peek_binary_object_name(&key)
                && !name.is_empty()
            {
                names_set.insert(name.to_ascii_lowercase());
                continue;
            }

            if let Ok(parsed) = binary.read()
                && let Ok(texture) = converter.from_unity_object(&parsed)
            {
                let name = texture.name;
                if !name.is_empty() {
                    names_set.insert(name.to_ascii_lowercase());
                }
            }
        }

        let mut container_paths: HashSet<String> = HashSet::new();
        for (path, _) in crate::assets::packed_textures::collect_container_paths(&env) {
            insert_with_ancestors(&mut container_paths, &path);
        }

        let mut names: Vec<String> = names_set.into_iter().collect();
        names.sort();

        Ok(Self {
            signature,
            names,
            container_paths,
        })
    }

    fn save(&self, cache_path: &Path) -> Result<()> {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut out = String::new();
        out.push_str("STITCHLANDS_PACKED_INDEX_V3\n");
        out.push_str(&self.signature);
        out.push('\n');
        out.push_str(&format!("{}\n", self.names.len()));
        for name in &self.names {
            out.push_str(name);
            out.push('\n');
        }
        let mut paths: Vec<&String> = self.container_paths.iter().collect();
        paths.sort();
        out.push_str(&format!("{}\n", paths.len()));
        for path in paths {
            out.push_str(path);
            out.push('\n');
        }

        fs::write(cache_path, out)?;
        Ok(())
    }

    fn load(cache_path: &Path) -> Result<Self> {
        let input = fs::read_to_string(cache_path)?;
        let mut lines = input.lines();

        let version = lines.next().unwrap_or_default();
        if version != "STITCHLANDS_PACKED_INDEX_V3" {
            anyhow::bail!("unsupported packed index version");
        }

        let signature = lines.next().unwrap_or_default().to_string();
        let names_len = lines.next().unwrap_or("0").parse::<usize>().unwrap_or(0);

        let mut names = Vec::with_capacity(names_len);
        for _ in 0..names_len {
            if let Some(line) = lines.next() {
                names.push(line.to_string());
            }
        }

        let container_len = lines.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
        let mut container_paths = HashSet::with_capacity(container_len);
        for _ in 0..container_len {
            if let Some(line) = lines.next() {
                container_paths.insert(line.to_string());
            }
        }

        Ok(Self {
            signature,
            names,
            container_paths,
        })
    }
}

fn roots_signature(roots: &[PathBuf], typetree_registries: &[PathBuf]) -> String {
    let mut parts = Vec::new();
    for root in roots {
        let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        let mtime = std::fs::metadata(root)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        parts.push(format!("data:{}|{}", canonical.display(), mtime));
    }
    for registry in typetree_registries {
        let canonical = std::fs::canonicalize(registry).unwrap_or_else(|_| registry.clone());
        let mtime = std::fs::metadata(registry)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        parts.push(format!("typetree:{}|{}", canonical.display(), mtime));
    }
    parts.sort();
    parts.join(";")
}

fn insert_with_ancestors(set: &mut HashSet<String>, path: &str) {
    set.insert(path.to_string());
    let mut remaining = path;
    while let Some(idx) = remaining.rfind('/') {
        remaining = &remaining[..idx];
        if remaining.is_empty() {
            break;
        }
        if !set.insert(remaining.to_string()) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;

    use super::PackedTextureIndex;

    #[test]
    fn persists_and_reloads_metadata_index() {
        let root = std::env::temp_dir().join(format!(
            "stitchlands-packed-index-persist-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("cache.txt");

        let first =
            PackedTextureIndex::load_or_build(std::slice::from_ref(&root), &[], &cache_path, false)
                .unwrap();
        let second =
            PackedTextureIndex::load_or_build(std::slice::from_ref(&root), &[], &cache_path, false)
                .unwrap();

        assert!(cache_path.exists());
        assert_eq!(
            first.search_names("steel", 10),
            second.search_names("steel", 10)
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_signature_triggers_rebuild() {
        let root = std::env::temp_dir().join(format!(
            "stitchlands-packed-index-stale-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("cache.txt");

        let stale = [
            "STITCHLANDS_PACKED_INDEX_V3",
            "stale_signature",
            "1",
            "made_up_texture",
            "0",
            "",
        ]
        .join("\n");
        fs::write(&cache_path, stale).unwrap();

        let rebuilt =
            PackedTextureIndex::load_or_build(std::slice::from_ref(&root), &[], &cache_path, false)
                .unwrap();
        assert!(rebuilt.search_names("made_up_texture", 5).is_empty());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn maybe_contains_uses_name_and_container_metadata() {
        let root = std::env::temp_dir().join(format!(
            "stitchlands-packed-index-contains-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("cache.txt");

        let input = [
            "STITCHLANDS_PACKED_INDEX_V3",
            "any",
            "1",
            "steel_a",
            "1",
            "textures/things/pawn/humanlike/bodies/naked_male.png",
            "",
        ]
        .join("\n");
        fs::write(&cache_path, input).unwrap();

        let index = PackedTextureIndex::load(&cache_path).unwrap();
        assert!(index.maybe_contains("Things/Item/Resource/Steel"));
        assert!(index.maybe_contains("Things/Pawn/Humanlike/Bodies/Naked_Male"));
        assert!(!index.maybe_contains("Things/Item/Resource/DefinitelyMissing"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn empty_index_fails_open_for_lookup() {
        let root = std::env::temp_dir().join(format!(
            "stitchlands-packed-index-empty-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let cache_path = root.join("cache.txt");

        let input = ["STITCHLANDS_PACKED_INDEX_V3", "any", "0", "0", ""].join("\n");
        fs::write(&cache_path, input).unwrap();

        let index = PackedTextureIndex::load(&cache_path).unwrap();
        assert!(index.is_empty());
        assert!(index.maybe_contains("Things/Item/Resource/DefinitelyMissing"));

        let _ = fs::remove_dir_all(root);
    }

    fn index_with_names(names: &[&str]) -> PackedTextureIndex {
        PackedTextureIndex {
            signature: String::new(),
            names: {
                let mut v: Vec<String> = names.iter().map(|s| s.to_string()).collect();
                v.sort();
                v
            },
            container_paths: HashSet::new(),
        }
    }

    #[test]
    fn prefix_match_finds_variant_suffix() {
        let index = index_with_names(&["steel_a"]);
        assert!(index.maybe_contains("Things/Item/Resource/Steel"));
    }

    #[test]
    fn prefix_match_no_false_positive_without_underscore() {
        let index = index_with_names(&["steelwork"]);
        assert!(!index.maybe_contains("Things/Item/Resource/Steel"));
    }

    #[test]
    fn prefix_match_compound_suffix() {
        let index = index_with_names(&["agave_immature"]);
        assert!(index.maybe_contains("Things/Plant/Agave"));
    }

    #[test]
    fn prefix_match_clean_miss() {
        let index = index_with_names(&["steel_a", "wood_a"]);
        assert!(!index.maybe_contains("Things/Item/Resource/Gold"));
    }
}
