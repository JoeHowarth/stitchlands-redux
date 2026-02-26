use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use image::RgbaImage;
use log::{info, warn};
use unity_asset::environment::{BinaryObjectKey, Environment, EnvironmentObjectRef};
use unity_asset_core::constants::class_ids;
use unity_asset_decode::texture::Texture2DConverter;
use unity_asset_decode::unity_version::UnityVersion;

use crate::typetree_registry::load_typetree_registry;

pub struct PackedTextureResolver {
    env: Environment,
    converter: Texture2DConverter,
    keys_by_name: HashMap<String, BinaryObjectKey>,
    container_index: Vec<(String, BinaryObjectKey)>,
    loaded_roots: Vec<PathBuf>,
}

pub struct PackedTextureHit {
    pub image: RgbaImage,
    pub source_label: String,
    pub matched_name: String,
}

pub struct PackedProbeSummary {
    pub attempted: usize,
    pub succeeded: usize,
    pub sample_errors: Vec<(String, String)>,
}

pub struct PackedTextureExtractionSummary {
    pub scanned_textures: usize,
    pub exported_textures: usize,
    pub failed_textures: usize,
}

pub struct PackedDecodeHealth {
    pub attempted: usize,
    pub succeeded: usize,
    pub sample_errors: Vec<String>,
}

impl PackedTextureResolver {
    pub fn build(roots: &[PathBuf], typetree_registries: &[PathBuf]) -> Result<Option<Self>> {
        let existing_roots: Vec<PathBuf> = roots.iter().filter(|r| r.exists()).cloned().collect();
        if existing_roots.is_empty() {
            return Ok(None);
        }

        let mut env = Environment::new();
        if let Some(registry) = load_typetree_registry(typetree_registries)? {
            env.set_type_tree_registry(Some(registry));
        }
        for root in &existing_roots {
            if let Err(err) = env.load(root) {
                warn!("failed to load packed root {}: {err}", root.display());
            } else {
                info!("loaded packed data root: {}", root.display());
            }
        }

        let converter = Texture2DConverter::new(UnityVersion::default());
        let mut keys_by_name = HashMap::new();
        let mut texture_count = 0usize;
        let mut parsed_name_count = 0usize;
        for obj in env.objects() {
            let EnvironmentObjectRef::Binary(binary) = obj else {
                continue;
            };
            if binary.object.class_id() != class_ids::TEXTURE_2D {
                continue;
            }

            texture_count += 1;
            let key = binary.key();
            let name = match env.peek_binary_object_name(&key) {
                Ok(Some(name)) => Some(name),
                _ => match binary.read() {
                    Ok(parsed) => match converter.from_unity_object(&parsed) {
                        Ok(texture) => {
                            let name = texture.name;
                            if !name.is_empty() && name != "UnknownTexture" {
                                parsed_name_count += 1;
                                Some(name)
                            } else {
                                None
                            }
                        }
                        Err(_) => None,
                    },
                    Err(_) => None,
                },
            };
            if let Some(name) = name {
                if !name.is_empty() {
                    keys_by_name
                        .entry(name.to_ascii_lowercase())
                        .or_insert(key.clone());
                }
            }
        }

        let container_index: Vec<(String, BinaryObjectKey)> = env
            .find_binary_object_keys_in_bundle_container("")
            .into_iter()
            .map(|(path, key)| (path.to_ascii_lowercase(), key))
            .collect();

        info!(
            "packed texture index built: {} Texture2D objects, {} named entries, {} container entries",
            texture_count,
            keys_by_name.len(),
            container_index.len()
        );
        if parsed_name_count > 0 {
            info!(
                "packed texture names recovered via full parse: {}",
                parsed_name_count
            );
        }

        Ok(Some(Self {
            env,
            converter,
            keys_by_name,
            container_index,
            loaded_roots: existing_roots,
        }))
    }

    pub fn loaded_roots(&self) -> &[PathBuf] {
        &self.loaded_roots
    }

    pub fn search_names(&self, query: &str, limit: usize) -> Vec<String> {
        let needle = query.to_ascii_lowercase();
        let mut matches: Vec<String> = self
            .keys_by_name
            .keys()
            .filter(|name| name.contains(&needle))
            .take(limit)
            .cloned()
            .collect();
        matches.sort();
        matches
    }

    pub fn probe_decode_candidates(&self, tex_path: &str, limit: usize) -> PackedProbeSummary {
        let mut candidates: Vec<(String, BinaryObjectKey)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for wanted in wanted_texture_names(tex_path) {
            if let Some(key) = self.keys_by_name.get(&wanted) {
                if seen.insert((key.source.describe(), key.path_id)) {
                    candidates.push((wanted, key.clone()));
                }
            }
        }
        for (name, key) in self.find_fuzzy_name_matches(tex_path) {
            if seen.insert((key.source.describe(), key.path_id)) {
                candidates.push((name, key));
            }
            if candidates.len() >= limit {
                break;
            }
        }

        let mut succeeded = 0usize;
        let mut sample_errors = Vec::new();
        for (name, key) in candidates.iter().take(limit) {
            match self.decode_texture_for_key(key) {
                Ok(_) => succeeded += 1,
                Err(err) => {
                    if sample_errors.len() < 5 {
                        sample_errors.push((name.clone(), err.to_string()));
                    }
                }
            }
        }

        PackedProbeSummary {
            attempted: candidates.len().min(limit),
            succeeded,
            sample_errors,
        }
    }

    pub fn resolve(&self, tex_path: &str) -> Result<Option<PackedTextureHit>> {
        for wanted in wanted_texture_names(tex_path) {
            let Some(key) = self.keys_by_name.get(&wanted) else {
                continue;
            };

            let image = match self.decode_texture_for_key(key) {
                Ok(image) => image,
                Err(_) => continue,
            };
            let source_label = format!("{}::{}", key.source.describe(), key.path_id);

            return Ok(Some(PackedTextureHit {
                image,
                source_label,
                matched_name: wanted,
            }));
        }

        for (matched_name, key) in self.find_fuzzy_name_matches(tex_path) {
            let image = match self.decode_texture_for_key(&key) {
                Ok(image) => image,
                Err(_) => continue,
            };
            let source_label = format!("{}::{}", key.source.describe(), key.path_id);
            return Ok(Some(PackedTextureHit {
                image,
                source_label,
                matched_name,
            }));
        }

        for (asset_path, key) in self.find_by_container_paths(tex_path) {
            let image = match self.decode_texture_for_key(&key) {
                Ok(image) => image,
                Err(_) => continue,
            };
            let source_label = format!("{}::{}", key.source.describe(), key.path_id);

            return Ok(Some(PackedTextureHit {
                image,
                source_label,
                matched_name: asset_path,
            }));
        }

        Ok(None)
    }

    pub fn decode_health_sample(&self, sample_limit: usize) -> PackedDecodeHealth {
        let mut names: Vec<_> = self.keys_by_name.keys().cloned().collect();
        names.sort();

        let mut attempted = 0usize;
        let mut succeeded = 0usize;
        let mut sample_errors = Vec::new();

        for name in names.into_iter().take(sample_limit) {
            let Some(key) = self.keys_by_name.get(&name) else {
                continue;
            };
            attempted += 1;
            match self.decode_texture_for_key(key) {
                Ok(_) => succeeded += 1,
                Err(err) => {
                    if sample_errors.len() < 5 {
                        sample_errors.push(format!("{name}: {err}"));
                    }
                }
            }
        }

        PackedDecodeHealth {
            attempted,
            succeeded,
            sample_errors,
        }
    }

    fn find_fuzzy_name_matches(&self, tex_path: &str) -> Vec<(String, BinaryObjectKey)> {
        let basename = tex_path
            .rsplit('/')
            .next()
            .unwrap_or(tex_path)
            .to_ascii_lowercase();
        let mut matches: Vec<(i32, String, BinaryObjectKey)> = Vec::new();

        for (name, key) in &self.keys_by_name {
            if !name.contains(&basename) {
                continue;
            }
            let score = if name == &basename {
                100
            } else if name.starts_with(&basename) {
                80
            } else {
                50
            } - (name.len() as i32 / 20);

            matches.push((score, name.clone(), key.clone()));
        }
        matches.sort_by(|a, b| b.0.cmp(&a.0));
        matches
            .into_iter()
            .take(10)
            .map(|(_, name, key)| (name, key))
            .collect()
    }

    fn decode_texture_for_key(&self, key: &BinaryObjectKey) -> Result<RgbaImage> {
        let obj = self.env.read_binary_object_key(key)?;
        let mut texture = self.converter.from_unity_object(&obj)?;
        if texture.image_data.is_empty()
            && !texture.stream_info.path.is_empty()
            && texture.stream_info.size > 0
        {
            if let Ok(bytes) = self.env.read_stream_data_source(
                &key.source,
                key.source_kind,
                &texture.stream_info.path,
                texture.stream_info.offset,
                texture.stream_info.size,
            ) {
                texture.data_size = bytes.len() as i32;
                texture.image_data = bytes;
            }
        }
        let image = self.converter.decode_to_image(&texture)?;
        Ok(image)
    }

    fn find_by_container_paths(&self, tex_path: &str) -> Vec<(String, BinaryObjectKey)> {
        let candidates = wanted_container_patterns(tex_path);
        let mut matches: Vec<(i32, String, BinaryObjectKey)> = Vec::new();

        for (asset_path, key) in &self.container_index {
            for candidate in &candidates {
                if !asset_path.contains(candidate) {
                    continue;
                }

                let score = container_match_score(asset_path, candidate);
                matches.push((score, asset_path.clone(), key.clone()));
            }
        }

        matches.sort_by(|a, b| b.0.cmp(&a.0));
        matches
            .into_iter()
            .take(10)
            .map(|(_, path, key)| (path, key))
            .collect()
    }
}

pub fn extract_all_packed_textures(
    packed_roots: &[PathBuf],
    typetree_registries: &[PathBuf],
    output_dir: &Path,
) -> Result<PackedTextureExtractionSummary> {
    let existing_roots: Vec<PathBuf> = packed_roots
        .iter()
        .filter(|p| p.exists())
        .cloned()
        .collect();
    if existing_roots.is_empty() {
        anyhow::bail!("no existing packed roots were provided");
    }

    std::fs::create_dir_all(output_dir)?;

    let mut env = Environment::new();
    if let Some(registry) = load_typetree_registry(typetree_registries)? {
        env.set_type_tree_registry(Some(registry));
    }
    for root in &existing_roots {
        if let Err(err) = env.load(root) {
            warn!("failed to load packed root {}: {err}", root.display());
        }
    }

    let converter = Texture2DConverter::new(UnityVersion::default());
    let mut scanned = 0usize;
    let mut exported = 0usize;
    let mut failed = 0usize;

    for obj in env.objects() {
        let EnvironmentObjectRef::Binary(binary) = obj else {
            continue;
        };
        if binary.object.class_id() != class_ids::TEXTURE_2D {
            continue;
        }

        scanned += 1;
        let key = binary.key();
        let parsed = match binary.read() {
            Ok(parsed) => parsed,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        let mut texture = match converter.from_unity_object(&parsed) {
            Ok(texture) => texture,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        if texture.image_data.is_empty()
            && !texture.stream_info.path.is_empty()
            && texture.stream_info.size > 0
        {
            if let Ok(bytes) = env.read_stream_data_source(
                &key.source,
                key.source_kind,
                &texture.stream_info.path,
                texture.stream_info.offset,
                texture.stream_info.size,
            ) {
                texture.data_size = bytes.len() as i32;
                texture.image_data = bytes;
            }
        }
        let image = match converter.decode_to_image(&texture) {
            Ok(image) => image,
            Err(_) => {
                failed += 1;
                continue;
            }
        };

        let texture_name = parsed
            .name()
            .unwrap_or_else(|| format!("pathid_{}", key.path_id));
        let safe_name = sanitize_filename(&texture_name);
        let output_path = output_dir.join(format!("{}_{}.png", safe_name, key.path_id));
        match image.save(&output_path) {
            Ok(_) => exported += 1,
            Err(_) => failed += 1,
        }
    }

    Ok(PackedTextureExtractionSummary {
        scanned_textures: scanned,
        exported_textures: exported,
        failed_textures: failed,
    })
}

pub fn infer_packed_data_roots(input_path: &Path, data_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    roots.push(
        input_path
            .join("RimWorldMac.app")
            .join("Contents")
            .join("Resources")
            .join("Data"),
    );
    roots.push(input_path.join("Contents").join("Resources").join("Data"));
    roots.push(data_dir.join("Contents").join("Resources").join("Data"));
    if let Some(parent) = data_dir.parent() {
        roots.push(parent.join("Contents").join("Resources").join("Data"));
    }

    dedupe_paths(&mut roots);
    roots
}

fn wanted_texture_names(tex_path: &str) -> Vec<String> {
    let basename = tex_path
        .rsplit('/')
        .next()
        .unwrap_or(tex_path)
        .to_ascii_lowercase();
    vec![
        basename.clone(),
        format!("{}_south", basename),
        format!("{}_north", basename),
        format!("{}_east", basename),
        format!("{}_west", basename),
    ]
}

fn wanted_container_patterns(tex_path: &str) -> Vec<String> {
    let base = tex_path.to_ascii_lowercase();
    vec![
        base.clone(),
        format!("{base}.png"),
        format!("{base}_south"),
        format!("{base}_north"),
        format!("{base}_east"),
        format!("{base}_west"),
    ]
}

fn container_match_score(asset_path: &str, candidate: &str) -> i32 {
    let mut score = 0;
    if asset_path.ends_with(candidate) {
        score += 120;
    }
    if asset_path.contains("/things/") {
        score += 20;
    }
    if asset_path.contains(".png") {
        score += 10;
    }
    score - asset_path.len() as i32 / 100
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut unique = Vec::new();
    for path in std::mem::take(paths) {
        if unique.iter().any(|p: &PathBuf| p == &path) {
            continue;
        }
        unique.push(path);
    }
    *paths = unique;
}

fn sanitize_filename(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "texture".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wanted_names_include_directional_suffixes() {
        let names = wanted_texture_names("Things/Item/Resource/Steel");
        assert_eq!(names[0], "steel");
        assert!(names.contains(&"steel_south".to_string()));
        assert!(names.contains(&"steel_north".to_string()));
        assert!(names.contains(&"steel_east".to_string()));
        assert!(names.contains(&"steel_west".to_string()));
    }

    #[test]
    fn wanted_container_patterns_include_png_and_rotations() {
        let patterns = wanted_container_patterns("Things/Item/Resource/Steel");
        assert!(patterns.contains(&"things/item/resource/steel".to_string()));
        assert!(patterns.contains(&"things/item/resource/steel.png".to_string()));
        assert!(patterns.contains(&"things/item/resource/steel_south".to_string()));
    }

    #[test]
    fn sanitizes_filename() {
        assert_eq!(sanitize_filename("Things/Item/Steel"), "Things_Item_Steel");
        assert_eq!(sanitize_filename("   "), "texture");
    }
}
