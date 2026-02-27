use std::path::PathBuf;

use anyhow::{Context, Result};

mod default_config;
mod loose;
pub(crate) mod packed_index;
mod packed_textures;
mod resolver;
mod rimworld_paths;
mod typetree_registry;

pub use loose::{SpriteAsset, resolve_sprite, resolve_texture_path};
pub use packed_textures::{extract_all_packed_textures, infer_packed_data_roots};
pub use resolver::AssetResolver;

pub use self::default_config::default_packed_index_path;

pub struct AssetSetup {
    pub data_dir: PathBuf,
    pub typetree_registries: Vec<PathBuf>,
    pub resolver: AssetResolver,
}

pub struct AssetSetupOptions {
    pub rimworld_data: Option<PathBuf>,
    pub texture_roots: Vec<PathBuf>,
    pub packed_roots: Vec<PathBuf>,
    pub typetree_registry: Vec<PathBuf>,
    pub auto_typetree: bool,
    pub packed_index_path: Option<PathBuf>,
    pub rebuild_packed_index: bool,
    pub disable_packed_index: bool,
}

pub fn prepare_asset_setup(options: AssetSetupOptions) -> Result<AssetSetup> {
    let rimworld_input = default_config::resolve_rimworld_input(options.rimworld_data).context(
        "could not resolve RimWorld path; set --rimworld-data or STITCHLANDS_RIMWORLD_DATA",
    )?;
    let data_dir = rimworld_paths::resolve_data_dir(&rimworld_input).with_context(|| {
        format!(
            "resolving rimworld data dir from {}",
            rimworld_input.display()
        )
    })?;

    let texture_roots =
        default_config::merge_path_list(&options.texture_roots, "STITCHLANDS_TEXTURE_ROOT");
    let packed_root_overrides =
        default_config::merge_path_list(&options.packed_roots, "STITCHLANDS_PACKED_DATA_ROOT");
    let mut packed_roots = infer_packed_data_roots(&rimworld_input, &data_dir);
    packed_roots.extend(packed_root_overrides);
    packed_roots.sort();
    packed_roots.dedup();

    let typetree_registries = typetree_registry::resolve_typetree_registry_paths(
        &options.typetree_registry,
        options.auto_typetree,
    );

    let packed_index = if options.disable_packed_index {
        None
    } else {
        let cache_path = options
            .packed_index_path
            .unwrap_or_else(default_packed_index_path);
        Some(packed_index::PackedTextureIndex::load_or_build(
            &packed_roots,
            &typetree_registries,
            &cache_path,
            options.rebuild_packed_index,
        )?)
    };

    let resolver = AssetResolver::new(
        texture_roots.clone(),
        packed_roots.clone(),
        typetree_registries.clone(),
        packed_index,
    );

    Ok(AssetSetup {
        data_dir,
        typetree_registries,
        resolver,
    })
}
