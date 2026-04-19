use std::path::{Path, PathBuf};

use anyhow::Result;
use log::info;

use crate::assets::packed_index::PackedTextureIndex;
use crate::assets::packed_textures::{PackedProbeSummary, PackedTextureResolver};
use crate::assets::{SpriteAsset, resolve_sprite, resolve_texture_path};
use crate::defs::ThingDef;

pub struct AssetResolver {
    texture_roots: Vec<PathBuf>,
    packed_roots: Vec<PathBuf>,
    typetree_registries: Vec<PathBuf>,
    packed_index: Option<PackedTextureIndex>,
    packed_resolver_state: PackedResolverState,
}

impl AssetResolver {
    pub fn new(
        texture_roots: Vec<PathBuf>,
        packed_roots: Vec<PathBuf>,
        typetree_registries: Vec<PathBuf>,
        packed_index: Option<PackedTextureIndex>,
    ) -> Self {
        let packed_resolver_state =
            PackedResolverState::new(packed_roots.clone(), typetree_registries.clone());
        Self {
            texture_roots,
            packed_roots,
            typetree_registries,
            packed_index,
            packed_resolver_state,
        }
    }

    pub fn texture_roots(&self) -> &[PathBuf] {
        &self.texture_roots
    }

    pub fn packed_roots(&self) -> &[PathBuf] {
        &self.packed_roots
    }

    pub fn typetree_registries(&self) -> &[PathBuf] {
        &self.typetree_registries
    }

    pub fn search_packed_names(&self, query: &str, limit: usize) -> Option<Vec<String>> {
        self.packed_index
            .as_ref()
            .map(|index| index.search_names(query, limit))
    }

    pub fn resolve_thing(
        &mut self,
        data_dir: &Path,
        thing_def: &ThingDef,
    ) -> Result<ResolvedSpriteAsset> {
        let mut sprite_asset = resolve_sprite(data_dir, thing_def, &self.texture_roots)?;
        let mut resolved_from_packed = false;
        if sprite_asset.used_fallback {
            resolved_from_packed = self.try_resolve_from_packed(
                thing_def.graphic_data.tex_path.as_str(),
                &mut sprite_asset,
            )?;
        }

        Ok(ResolvedSpriteAsset {
            sprite: sprite_asset,
            resolved_from_packed,
        })
    }

    pub fn resolve_texture_path(
        &mut self,
        data_dir: &Path,
        tex_path: &str,
    ) -> Result<ResolvedSpriteAsset> {
        let mut sprite_asset = resolve_texture_path(data_dir, tex_path, &self.texture_roots)?;
        let mut resolved_from_packed = false;
        if sprite_asset.used_fallback {
            resolved_from_packed = self.try_resolve_from_packed(tex_path, &mut sprite_asset)?;
        }

        Ok(ResolvedSpriteAsset {
            sprite: sprite_asset,
            resolved_from_packed,
        })
    }

    pub fn maybe_probe_decode_candidates(
        &mut self,
        tex_path: &str,
        limit: usize,
    ) -> Result<Option<PackedProbeSummary>> {
        if !self.can_try_packed(tex_path) {
            return Ok(None);
        }
        let probe = self
            .packed_resolver_state
            .get()?
            .map(|resolver| resolver.probe_decode_candidates(tex_path, limit));
        Ok(probe)
    }

    pub fn probe_folder_variant(
        &mut self,
        tex_path: &str,
        variant_index: usize,
    ) -> Result<Option<String>> {
        let Some(resolver) = self.packed_resolver_state.get()? else {
            return Ok(None);
        };
        match resolver.resolve_folder_variant(tex_path, variant_index)? {
            Some(hit) => Ok(Some(hit.source_label)),
            None => Ok(None),
        }
    }

    pub fn search_packed_container(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Option<Vec<String>>> {
        let Some(resolver) = self.packed_resolver_state.get()? else {
            return Ok(None);
        };
        Ok(Some(resolver.search_container_paths(query, limit)))
    }

    pub fn run_packed_class_probe(&mut self, sample_limit: usize) -> Result<bool> {
        let Some(resolver) = self.packed_resolver_state.get()? else {
            return Ok(false);
        };
        resolver.run_class_id_probe(sample_limit);
        Ok(true)
    }

    pub fn run_packed_decode_probe(
        &mut self,
        sample_limit: usize,
        min_attempts: usize,
    ) -> Result<Option<PackedDecodeProbeOutcome>> {
        let Some(resolver) = self.packed_resolver_state.get()? else {
            return Ok(None);
        };

        let health = resolver.decode_health_sample(sample_limit);
        if health.attempted < min_attempts {
            return Ok(None);
        }

        let disable_packed = health.succeeded == 0;
        let outcome = PackedDecodeProbeOutcome {
            attempted: health.attempted,
            succeeded: health.succeeded,
            sample_errors: health.sample_errors,
            disable_packed,
        };

        if disable_packed {
            self.packed_resolver_state.disable();
        }

        Ok(Some(outcome))
    }

    pub fn can_try_packed(&self, tex_path: &str) -> bool {
        self.packed_index
            .as_ref()
            .map(|index| index.maybe_contains(tex_path))
            .unwrap_or(true)
    }

    fn try_resolve_from_packed(
        &mut self,
        tex_path: &str,
        sprite_asset: &mut SpriteAsset,
    ) -> Result<bool> {
        if !self.can_try_packed(tex_path) {
            return Ok(false);
        }

        let Some(resolver) = self.packed_resolver_state.get()? else {
            return Ok(false);
        };

        let Ok(Some(hit)) = resolver.resolve(tex_path) else {
            return Ok(false);
        };

        sprite_asset.image = hit.image;
        sprite_asset.source_path = Some(PathBuf::from(hit.source_label));
        sprite_asset.used_fallback = false;
        Ok(true)
    }
}

pub struct ResolvedSpriteAsset {
    pub sprite: SpriteAsset,
    pub resolved_from_packed: bool,
}

pub struct PackedDecodeProbeOutcome {
    pub attempted: usize,
    pub succeeded: usize,
    pub sample_errors: Vec<String>,
    pub disable_packed: bool,
}

struct PackedResolverState {
    packed_roots: Vec<PathBuf>,
    typetree_registries: Vec<PathBuf>,
    resolver: Option<PackedTextureResolver>,
    build_attempted: bool,
    build_fn: Box<BuildPackedResolverFn>,
}

type BuildPackedResolverFn =
    dyn FnMut(&[PathBuf], &[PathBuf]) -> Result<Option<PackedTextureResolver>>;

impl PackedResolverState {
    fn new(packed_roots: Vec<PathBuf>, typetree_registries: Vec<PathBuf>) -> Self {
        Self {
            packed_roots,
            typetree_registries,
            resolver: None,
            build_attempted: false,
            build_fn: Box::new(PackedTextureResolver::build),
        }
    }

    #[cfg(test)]
    fn with_builder(
        packed_roots: Vec<PathBuf>,
        typetree_registries: Vec<PathBuf>,
        build_fn: Box<BuildPackedResolverFn>,
    ) -> Self {
        Self {
            packed_roots,
            typetree_registries,
            resolver: None,
            build_attempted: false,
            build_fn,
        }
    }

    fn get(&mut self) -> Result<Option<&PackedTextureResolver>> {
        if !self.build_attempted {
            self.resolver = (self.build_fn)(&self.packed_roots, &self.typetree_registries)?;
            self.build_attempted = true;
            if let Some(resolver) = self.resolver.as_ref() {
                for root in resolver.loaded_roots() {
                    info!("packed resolver root: {}", root.display());
                }
            }
        }
        Ok(self.resolver.as_ref())
    }

    fn disable(&mut self) {
        self.build_attempted = true;
        self.resolver = None;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::PackedResolverState;

    #[test]
    fn lazy_builder_runs_once_when_result_is_none() {
        let calls = Rc::new(RefCell::new(0usize));
        let calls_for_builder = Rc::clone(&calls);
        let mut state = PackedResolverState::with_builder(
            vec![],
            vec![],
            Box::new(move |_, _| {
                *calls_for_builder.borrow_mut() += 1;
                Ok(None)
            }),
        );

        let _ = state.get().unwrap();
        let _ = state.get().unwrap();
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn disable_prevents_builder_execution() {
        let calls = Rc::new(RefCell::new(0usize));
        let calls_for_builder = Rc::clone(&calls);
        let mut state = PackedResolverState::with_builder(
            vec![],
            vec![],
            Box::new(move |_, _| {
                *calls_for_builder.borrow_mut() += 1;
                Ok(None)
            }),
        );

        state.disable();
        let _ = state.get().unwrap();
        assert_eq!(*calls.borrow(), 0);
    }
}
