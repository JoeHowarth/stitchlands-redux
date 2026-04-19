use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use log::{info, warn};

use crate::assets::{AssetResolver, AssetSetupOptions, prepare_asset_setup};
use crate::cli::DataArgs;
use crate::defs::{
    ApparelDef, BeardDefRender, BodyTypeDefRender, HairDefRender, HeadTypeDefRender, TerrainDef,
    ThingDef, load_apparel_defs, load_beard_defs, load_body_type_defs, load_hair_defs,
    load_head_type_defs, load_humanlike_render_tree_layers, load_terrain_defs, load_thing_defs,
};
use crate::pawn::PawnComposeConfig;

pub struct AppContext {
    pub data_dir: PathBuf,
    pub thing_defs: HashMap<String, ThingDef>,
    pub terrain_defs: HashMap<String, TerrainDef>,
    pub apparel_defs: HashMap<String, ApparelDef>,
    pub body_type_defs: HashMap<String, BodyTypeDefRender>,
    pub head_type_defs: HashMap<String, HeadTypeDefRender>,
    pub beard_defs: HashMap<String, BeardDefRender>,
    pub hair_defs: HashMap<String, HairDefRender>,
    pub compose_config: PawnComposeConfig,
    pub asset_resolver: AssetResolver,
    pub allow_fallback: bool,
}

impl AppContext {
    pub fn load(
        data: &DataArgs,
        compose_from_layers: fn(crate::defs::HumanlikeRenderTreeLayers) -> PawnComposeConfig,
    ) -> Result<Self> {
        let setup = prepare_asset_setup(AssetSetupOptions {
            rimworld_data: data.rimworld_data.clone(),
            texture_roots: data.texture_root.clone(),
            packed_roots: data.packed_data_root.clone(),
            typetree_registry: data.typetree_registry.clone(),
            auto_typetree: data.auto_typetree,
            packed_index_path: data.packed_index_path.clone(),
            rebuild_packed_index: data.rebuild_packed_index,
        })?;

        let data_dir = setup.data_dir;
        info!("using RimWorld data dir: {}", data_dir.display());

        let thing_defs = load_thing_defs(&data_dir)
            .with_context(|| format!("loading defs from {}", data_dir.display()))?;
        info!("loaded {} thing defs with graphicData", thing_defs.len());

        let terrain_defs = load_terrain_defs(&data_dir)
            .with_context(|| format!("loading terrain defs from {}", data_dir.display()))?;
        info!(
            "loaded {} terrain defs with texturePath",
            terrain_defs.len()
        );

        let apparel_defs = load_apparel_defs(&data_dir)
            .with_context(|| format!("loading apparel defs from {}", data_dir.display()))?;
        info!(
            "loaded {} apparel defs with graphicData",
            apparel_defs.len()
        );

        let body_type_defs = load_body_type_defs(&data_dir)
            .with_context(|| format!("loading body type defs from {}", data_dir.display()))?;
        info!("loaded {} body type defs", body_type_defs.len());

        let head_type_defs = load_head_type_defs(&data_dir)
            .with_context(|| format!("loading head type defs from {}", data_dir.display()))?;
        info!("loaded {} head type defs", head_type_defs.len());

        let beard_defs = load_beard_defs(&data_dir)
            .with_context(|| format!("loading beard defs from {}", data_dir.display()))?;
        info!("loaded {} beard defs", beard_defs.len());

        let hair_defs = load_hair_defs(&data_dir)
            .with_context(|| format!("loading hair defs from {}", data_dir.display()))?;
        info!("loaded {} hair defs", hair_defs.len());

        let humanlike_layers = load_humanlike_render_tree_layers(&data_dir).with_context(|| {
            format!("loading humanlike render tree from {}", data_dir.display())
        })?;
        info!(
            "loaded humanlike render tree layers: body={} head={} beard={} hair={} apparel_body={} apparel_head={}",
            humanlike_layers.body_base_layer,
            humanlike_layers.head_base_layer,
            humanlike_layers.beard_base_layer,
            humanlike_layers.hair_base_layer,
            humanlike_layers.apparel_body_base_layer,
            humanlike_layers.apparel_head_base_layer
        );
        let compose_config = compose_from_layers(humanlike_layers);

        if setup.typetree_registries.is_empty() {
            warn!(
                "no typetree registry selected; packed texture decode may fail on stripped Unity assets (set --typetree-registry or STITCHLANDS_TYPETREE_REGISTRY)"
            );
        } else {
            for registry in &setup.typetree_registries {
                info!("using typetree registry: {}", registry.display());
            }
        }

        Ok(Self {
            data_dir,
            thing_defs,
            terrain_defs,
            apparel_defs,
            body_type_defs,
            head_type_defs,
            beard_defs,
            hair_defs,
            compose_config,
            asset_resolver: setup.resolver,
            allow_fallback: data.allow_fallback,
        })
    }
}
