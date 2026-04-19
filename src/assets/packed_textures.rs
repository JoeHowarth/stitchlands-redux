use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use image::RgbaImage;
use log::{info, warn};
use unity_asset::environment::{BinaryObjectKey, Environment, EnvironmentObjectRef};
use unity_asset_core::UnityValue;
use unity_asset_core::constants::class_ids;
use unity_asset_decode::texture::Texture2DConverter;
use unity_asset_decode::unity_version::UnityVersion;

use crate::assets::typetree_registry::load_typetree_registry;
use crate::assets::variants::variants_for;
use crate::defs::GraphicKind;

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
        let Some((env, existing_roots)) = build_packed_environment(roots, typetree_registries)?
        else {
            return Ok(None);
        };

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
            if let Some(name) = name
                && !name.is_empty()
            {
                keys_by_name
                    .entry(name.to_ascii_lowercase())
                    .or_insert(key.clone());
            }
        }

        let container_index = collect_container_paths(&env);

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

    pub fn probe_decode_candidates(&self, tex_path: &str, limit: usize) -> PackedProbeSummary {
        let mut candidates: Vec<(String, BinaryObjectKey)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let variants = variants_for(tex_path, GraphicKind::Single);
        for wanted in variants.base_names() {
            if let Some(key) = self.keys_by_name.get(&wanted)
                && seen.insert((key.source.describe(), key.path_id))
            {
                candidates.push((wanted, key.clone()));
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
        let variants = variants_for(tex_path, GraphicKind::Single);
        for wanted in variants.base_names() {
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
            }));
        }

        for (_asset_path, key) in self.find_by_container_paths(tex_path) {
            let image = match self.decode_texture_for_key(&key) {
                Ok(image) => image,
                Err(_) => continue,
            };
            let source_label = format!("{}::{}", key.source.describe(), key.path_id);

            return Ok(Some(PackedTextureHit {
                image,
                source_label,
            }));
        }

        Ok(None)
    }

    pub fn resolve_folder_variant(
        &self,
        tex_path: &str,
        variant_index: usize,
    ) -> Result<Option<PackedTextureHit>> {
        let members = self.folder_members(tex_path);
        if members.is_empty() {
            return Ok(None);
        }
        let (_asset_path, key) = &members[variant_index % members.len()];
        let image = self.decode_texture_for_key(key)?;
        let source_label = format!("{}::{}", key.source.describe(), key.path_id);
        Ok(Some(PackedTextureHit {
            image,
            source_label,
        }))
    }

    fn folder_members(&self, tex_path: &str) -> Vec<(String, BinaryObjectKey)> {
        let Some(base) = variants_for(tex_path, GraphicKind::Random).folder_prefix() else {
            return Vec::new();
        };
        let mut matches: Vec<(String, BinaryObjectKey)> = self
            .container_index
            .iter()
            .filter(|(path, _)| {
                let Some(tail) = path.strip_prefix(&base) else {
                    return false;
                };
                !tail.contains('/') && !tail.ends_with("_m")
            })
            .map(|(path, key)| (path.clone(), key.clone()))
            .collect();
        matches.sort_by(|a, b| a.0.cmp(&b.0));
        matches
    }

    pub fn search_container_paths(&self, query: &str, limit: usize) -> Vec<String> {
        let q = query.to_ascii_lowercase();
        self.container_index
            .iter()
            .filter(|(path, _)| path.contains(&q))
            .take(limit)
            .map(|(path, _)| path.clone())
            .collect()
    }

    pub fn run_class_id_probe(&self, sample_limit: usize) {
        let mut histogram: HashMap<i32, usize> = HashMap::new();
        for obj in self.env.objects() {
            let EnvironmentObjectRef::Binary(binary) = obj else {
                continue;
            };
            *histogram.entry(binary.object.class_id()).or_insert(0) += 1;
        }

        let mut rows: Vec<(i32, usize)> = histogram.into_iter().collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));

        info!(
            "packed class-id histogram ({} distinct classes):",
            rows.len()
        );
        for (class_id, count) in &rows {
            let name = unity_asset_core::get_class_name_str(*class_id).unwrap_or("Unknown");
            info!("  class {:>4} ({:<24}) = {}", class_id, name, count);
        }

        info!(
            "sampling up to {} class-147 (ResourceManager) objects:",
            sample_limit
        );
        let mut sampled = 0usize;
        for obj in self.env.objects() {
            let EnvironmentObjectRef::Binary(binary) = obj else {
                continue;
            };
            if binary.object.class_id() != 147 {
                continue;
            }
            sampled += 1;
            match binary.read() {
                Ok(parsed) => {
                    let field_names: Vec<&String> = parsed.class.property_names().collect();
                    info!("  class-147 #{sampled} fields: {:?}", field_names);
                    if let Some(UnityValue::Array(items)) = parsed.class.get("m_Container") {
                        info!(
                            "  class-147 #{sampled} m_Container entries: {}",
                            items.len()
                        );
                        for (i, item) in items.iter().take(5).enumerate() {
                            info!("    [{i}] {:?}", item);
                        }
                    } else {
                        info!("  class-147 #{sampled} has no m_Container field");
                    }
                }
                Err(e) => {
                    info!("  class-147 #{sampled} read error: {e}");
                }
            }
            if sampled >= sample_limit {
                break;
            }
        }
        info!("class-147 objects sampled: {}", sampled);
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

    fn decode_texture_for_key(&self, key: &BinaryObjectKey) -> Result<RgbaImage> {
        let obj = self.env.read_binary_object_key(key)?;
        let mut texture = self.converter.from_unity_object(&obj)?;
        if texture.image_data.is_empty()
            && !texture.stream_info.path.is_empty()
            && texture.stream_info.size > 0
            && let Ok(bytes) = self.env.read_stream_data_source(
                &key.source,
                key.source_kind,
                &texture.stream_info.path,
                texture.stream_info.offset,
                texture.stream_info.size,
            )
        {
            texture.data_size = bytes.len() as i32;
            texture.image_data = bytes;
        }
        let mut image = self.converter.decode_to_image(&texture)?;
        normalize_packed_texture_orientation(&mut image);
        Ok(image)
    }

    fn find_by_container_paths(&self, tex_path: &str) -> Vec<(String, BinaryObjectKey)> {
        let variants = variants_for(tex_path, GraphicKind::Single);
        let candidates = variants.container_paths();
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

const RESOURCE_MANAGER_CLASS_ID: i32 = 147;

/// Build a Unity `Environment` from the given packed roots. Returns `Some((env,
/// existing_roots))` when at least one root exists, `None` when all roots are
/// missing. Load failures of individual roots are warned; the typetree registry
/// (if any) is attached before loading.
pub(crate) fn build_packed_environment(
    roots: &[PathBuf],
    typetree_registries: &[PathBuf],
) -> Result<Option<(Environment, Vec<PathBuf>)>> {
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

    Ok(Some((env, existing_roots)))
}

pub(crate) fn collect_container_paths(env: &Environment) -> Vec<(String, BinaryObjectKey)> {
    let mut out: Vec<(String, BinaryObjectKey)> = env
        .find_binary_object_keys_in_bundle_container("")
        .into_iter()
        .map(|(path, key)| (path.to_ascii_lowercase(), key))
        .collect();
    out.extend(extract_resource_manager_container(env));
    out
}

fn extract_resource_manager_container(env: &Environment) -> Vec<(String, BinaryObjectKey)> {
    let mut out = Vec::new();
    for obj in env.objects() {
        let EnvironmentObjectRef::Binary(binary) = obj else {
            continue;
        };
        if binary.object.class_id() != RESOURCE_MANAGER_CLASS_ID {
            continue;
        }
        let parsed = match binary.read() {
            Ok(p) => p,
            Err(err) => {
                warn!("failed to read ResourceManager (class 147) object: {err}");
                continue;
            }
        };
        let Some(UnityValue::Array(items)) = parsed.class.get("m_Container") else {
            continue;
        };

        for item in items {
            let Some((path, pptr)) = extract_container_pair(item) else {
                continue;
            };
            let Some((file_id, path_id)) = scan_pptr(pptr) else {
                continue;
            };
            if path_id == 0 {
                continue;
            }
            if let Some(key) = env.resolve_binary_pptr(&binary, file_id, path_id) {
                out.push((path.to_ascii_lowercase(), key));
            }
        }
    }
    out
}

fn extract_container_pair(item: &UnityValue) -> Option<(String, &UnityValue)> {
    match item {
        UnityValue::Array(pair) if pair.len() == 2 => {
            let first = pair[0].as_str()?.to_string();
            Some((first, &pair[1]))
        }
        UnityValue::Object(obj) => {
            let first = obj.get("first").and_then(|v| v.as_str())?.to_string();
            let second = obj.get("second").or_else(|| obj.get("value"))?;
            Some((first, second))
        }
        _ => None,
    }
}

fn scan_pptr(value: &UnityValue) -> Option<(i32, i64)> {
    let UnityValue::Object(obj) = value else {
        return None;
    };
    let file_id = obj
        .get("fileID")
        .or_else(|| obj.get("m_FileID"))
        .and_then(|v| v.as_i64())
        .and_then(|v| i32::try_from(v).ok())?;
    let path_id = obj
        .get("pathID")
        .or_else(|| obj.get("m_PathID"))
        .and_then(|v| v.as_i64())?;
    Some((file_id, path_id))
}

pub fn extract_all_packed_textures(
    packed_roots: &[PathBuf],
    typetree_registries: &[PathBuf],
    output_dir: &Path,
) -> Result<PackedTextureExtractionSummary> {
    let Some((env, _)) = build_packed_environment(packed_roots, typetree_registries)? else {
        anyhow::bail!("no existing packed roots were provided");
    };

    std::fs::create_dir_all(output_dir)?;

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
            && let Ok(bytes) = env.read_stream_data_source(
                &key.source,
                key.source_kind,
                &texture.stream_info.path,
                texture.stream_info.offset,
                texture.stream_info.size,
            )
        {
            texture.data_size = bytes.len() as i32;
            texture.image_data = bytes;
        }
        let mut image = match converter.decode_to_image(&texture) {
            Ok(image) => image,
            Err(_) => {
                failed += 1;
                continue;
            }
        };
        normalize_packed_texture_orientation(&mut image);

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

fn normalize_packed_texture_orientation(image: &mut RgbaImage) {
    image::imageops::flip_vertical_in_place(image);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_filename() {
        assert_eq!(sanitize_filename("Things/Item/Steel"), "Things_Item_Steel");
        assert_eq!(sanitize_filename("   "), "texture");
    }
}
