use std::path::PathBuf;

use anyhow::Result;
use log::info;

use crate::assets::backend::{
    BackendLookup, ResolvedSprite, SpriteSource, TextureBackend, TextureQuery,
};
use crate::assets::loose::{LooseFiles, checkerboard_image};
use crate::assets::packed_index::PackedTextureIndex;
use crate::assets::packed_textures::{PackedProbeSummary, PackedTextureResolver};

pub struct AssetResolver {
    loose: LooseFiles,
    packed: PackedCatalog,
}

impl AssetResolver {
    pub fn new(
        core_data_dir: PathBuf,
        texture_roots: Vec<PathBuf>,
        packed_roots: Vec<PathBuf>,
        typetree_registries: Vec<PathBuf>,
        packed_index: PackedTextureIndex,
    ) -> Self {
        Self {
            loose: LooseFiles::new(core_data_dir, texture_roots),
            packed: PackedCatalog::new(packed_roots, typetree_registries, packed_index),
        }
    }

    pub fn texture_roots(&self) -> &[PathBuf] {
        self.loose.extra_texture_roots()
    }

    pub fn packed_roots(&self) -> &[PathBuf] {
        self.packed.packed_roots()
    }

    pub fn typetree_registries(&self) -> &[PathBuf] {
        self.packed.typetree_registries()
    }

    pub fn resolve(&mut self, query: TextureQuery) -> Result<ResolvedSprite> {
        if let BackendLookup::Hit(sprite) = self.loose.lookup(&query)? {
            return Ok(sprite);
        }

        if let BackendLookup::Hit(sprite) = self.packed.lookup(&query)? {
            return Ok(sprite);
        }

        Ok(ResolvedSprite {
            image: checkerboard_image(64, 64),
            source: SpriteSource::Fallback,
        })
    }

    pub fn resolve_texture_path(&mut self, tex_path: &str) -> Result<ResolvedSprite> {
        self.resolve(TextureQuery::single(tex_path))
    }

    pub fn resolve_thing(
        &mut self,
        thing_def: &crate::defs::ThingDef,
        variant_index: usize,
    ) -> Result<ResolvedSprite> {
        self.resolve(TextureQuery::for_thing(thing_def, variant_index))
    }

    pub fn search_packed_names(&self, query: &str, limit: usize) -> Vec<String> {
        self.packed.search_names(query, limit)
    }

    pub fn can_try_packed(&self, tex_path: &str) -> bool {
        self.packed.can_try(tex_path)
    }

    pub fn maybe_probe_decode_candidates(
        &mut self,
        tex_path: &str,
        limit: usize,
    ) -> Result<Option<PackedProbeSummary>> {
        self.packed.maybe_probe_decode_candidates(tex_path, limit)
    }

    pub fn probe_folder_variant(
        &mut self,
        tex_path: &str,
        variant_index: usize,
    ) -> Result<Option<String>> {
        self.packed.probe_folder_variant(tex_path, variant_index)
    }

    pub fn search_packed_container(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<Option<Vec<String>>> {
        self.packed.search_container_paths(query, limit)
    }

    pub fn run_packed_class_probe(&mut self, sample_limit: usize) -> Result<bool> {
        self.packed.run_class_probe(sample_limit)
    }

    pub fn run_packed_decode_probe(
        &mut self,
        sample_limit: usize,
        min_attempts: usize,
    ) -> Result<Option<PackedDecodeProbeOutcome>> {
        self.packed.run_decode_probe(sample_limit, min_attempts)
    }
}

pub struct PackedDecodeProbeOutcome {
    pub attempted: usize,
    pub succeeded: usize,
    pub sample_errors: Vec<String>,
    pub disable_packed: bool,
}

pub struct PackedCatalog {
    packed_roots: Vec<PathBuf>,
    typetree_registries: Vec<PathBuf>,
    index: PackedTextureIndex,
    resolver: Option<PackedTextureResolver>,
    build_attempted: bool,
    build_fn: Box<BuildPackedResolverFn>,
}

type BuildPackedResolverFn =
    dyn FnMut(&[PathBuf], &[PathBuf]) -> Result<Option<PackedTextureResolver>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackedPrefilter {
    Texture,
    Folder,
}

impl PackedCatalog {
    fn new(
        packed_roots: Vec<PathBuf>,
        typetree_registries: Vec<PathBuf>,
        index: PackedTextureIndex,
    ) -> Self {
        Self {
            packed_roots,
            typetree_registries,
            index,
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
            index: PackedTextureIndex::from_parts(&[], &[]),
            resolver: None,
            build_attempted: false,
            build_fn,
        }
    }

    fn packed_roots(&self) -> &[PathBuf] {
        &self.packed_roots
    }

    fn typetree_registries(&self) -> &[PathBuf] {
        &self.typetree_registries
    }

    fn search_names(&self, query: &str, limit: usize) -> Vec<String> {
        self.index.search_names(query, limit)
    }

    fn can_try(&self, tex_path: &str) -> bool {
        self.index.maybe_contains(tex_path)
    }

    fn can_try_query(&self, query: &TextureQuery) -> bool {
        match Self::prefilter_for(query) {
            PackedPrefilter::Folder => self.index.maybe_contains_folder(query.tex_path),
            PackedPrefilter::Texture => self.index.maybe_contains(query.tex_path),
        }
    }

    fn prefilter_for(query: &TextureQuery) -> PackedPrefilter {
        if query.kind.is_random() {
            PackedPrefilter::Folder
        } else {
            PackedPrefilter::Texture
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

    fn maybe_probe_decode_candidates(
        &mut self,
        tex_path: &str,
        limit: usize,
    ) -> Result<Option<PackedProbeSummary>> {
        if !self.can_try(tex_path) {
            return Ok(None);
        }
        let probe = self
            .get()?
            .map(|resolver| resolver.probe_decode_candidates(tex_path, limit));
        Ok(probe)
    }

    fn probe_folder_variant(
        &mut self,
        tex_path: &str,
        variant_index: usize,
    ) -> Result<Option<String>> {
        let Some(resolver) = self.get()? else {
            return Ok(None);
        };
        match resolver.resolve_folder_variant(tex_path, variant_index)? {
            Some(hit) => Ok(Some(hit.source_label)),
            None => Ok(None),
        }
    }

    fn search_container_paths(&mut self, query: &str, limit: usize) -> Result<Option<Vec<String>>> {
        let Some(resolver) = self.get()? else {
            return Ok(None);
        };
        Ok(Some(resolver.search_container_paths(query, limit)))
    }

    fn run_class_probe(&mut self, sample_limit: usize) -> Result<bool> {
        let Some(resolver) = self.get()? else {
            return Ok(false);
        };
        resolver.run_class_id_probe(sample_limit);
        Ok(true)
    }

    fn run_decode_probe(
        &mut self,
        sample_limit: usize,
        min_attempts: usize,
    ) -> Result<Option<PackedDecodeProbeOutcome>> {
        let Some(resolver) = self.get()? else {
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
            self.disable();
        }

        Ok(Some(outcome))
    }
}

impl TextureBackend for PackedCatalog {
    fn lookup(&mut self, query: &TextureQuery) -> Result<BackendLookup> {
        if !self.can_try_query(query) {
            return Ok(BackendLookup::Miss);
        }

        let Some(resolver) = self.get()? else {
            return Ok(BackendLookup::Miss);
        };

        let hit = if query.kind.is_random() {
            match resolver.resolve_folder_variant(query.tex_path, query.variant_index)? {
                Some(hit) => Some(hit),
                None => resolver.resolve(query.tex_path)?,
            }
        } else {
            resolver.resolve(query.tex_path)?
        };

        match hit {
            Some(hit) => Ok(BackendLookup::Hit(ResolvedSprite {
                image: hit.image,
                source: SpriteSource::Packed {
                    label: hit.source_label,
                },
            })),
            None => Ok(BackendLookup::Miss),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{PackedCatalog, PackedPrefilter};
    use crate::assets::backend::TextureQuery;
    use crate::assets::packed_index::PackedTextureIndex;
    use crate::defs::GraphicKind;

    #[test]
    fn lazy_builder_runs_once_when_result_is_none() {
        let calls = Rc::new(RefCell::new(0usize));
        let calls_for_builder = Rc::clone(&calls);
        let mut catalog = PackedCatalog::with_builder(
            vec![],
            vec![],
            Box::new(move |_, _| {
                *calls_for_builder.borrow_mut() += 1;
                Ok(None)
            }),
        );

        let _ = catalog.get().unwrap();
        let _ = catalog.get().unwrap();
        assert_eq!(*calls.borrow(), 1);
    }

    #[test]
    fn disable_prevents_builder_execution() {
        let calls = Rc::new(RefCell::new(0usize));
        let calls_for_builder = Rc::clone(&calls);
        let mut catalog = PackedCatalog::with_builder(
            vec![],
            vec![],
            Box::new(move |_, _| {
                *calls_for_builder.borrow_mut() += 1;
                Ok(None)
            }),
        );

        catalog.disable();
        let _ = catalog.get().unwrap();
        assert_eq!(*calls.borrow(), 0);
    }

    #[test]
    fn random_queries_select_folder_prefilter() {
        assert_eq!(
            PackedCatalog::prefilter_for(&TextureQuery {
                tex_path: "Things/Item/Chunk/ChunkSlag",
                kind: GraphicKind::Random,
                variant_index: 0,
            }),
            PackedPrefilter::Folder
        );
        assert_eq!(
            PackedCatalog::prefilter_for(&TextureQuery {
                tex_path: "Things/Item/Chunk/ChunkSlag",
                kind: GraphicKind::RandomRotated,
                variant_index: 0,
            }),
            PackedPrefilter::Folder
        );
    }

    #[test]
    fn single_queries_select_texture_prefilter() {
        assert_eq!(
            PackedCatalog::prefilter_for(&TextureQuery {
                tex_path: "Things/Item/Chunk/ChunkSlag",
                kind: GraphicKind::Single,
                variant_index: 0,
            }),
            PackedPrefilter::Texture
        );
        assert_eq!(
            PackedCatalog::prefilter_for(&TextureQuery {
                tex_path: "Things/Item/Resource/Steel",
                kind: GraphicKind::Multi,
                variant_index: 0,
            }),
            PackedPrefilter::Texture
        );
    }

    #[test]
    fn can_try_query_respects_selected_prefilter() {
        let mut catalog = PackedCatalog::with_builder(vec![], vec![], Box::new(|_, _| Ok(None)));
        catalog.index =
            PackedTextureIndex::from_parts(&["steel_a"], &["textures/things/item/chunk/chunkslag"]);

        assert!(catalog.can_try_query(&TextureQuery {
            tex_path: "Things/Item/Chunk/ChunkSlag",
            kind: GraphicKind::Random,
            variant_index: 0,
        }));
        assert!(catalog.can_try_query(&TextureQuery {
            tex_path: "Things/Item/Resource/Steel",
            kind: GraphicKind::Single,
            variant_index: 0,
        }));
        assert!(!catalog.can_try_query(&TextureQuery {
            tex_path: "Things/Item/Resource/Gold",
            kind: GraphicKind::Random,
            variant_index: 0,
        }));
    }
}
