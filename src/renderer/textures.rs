use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use anyhow::{Context, Result};
use image::RgbaImage;

use crate::assets::{AssetResolver, TextureQuery};
use crate::scene::{SceneTexture, SceneTextureTransform, TextureHandle};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct TextureKey {
    width: u32,
    height: u32,
    hash: u64,
}

pub(crate) struct TextureRegistry {
    pub(crate) layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<TextureHandle, TextureEntry>,
    path_textures: TexturePathCache,
    texture_keys: HashMap<TextureKey, TextureHandle>,
    next_texture_id: u32,
    upload_count: usize,
}

struct TextureEntry {
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

#[derive(Default)]
struct TexturePathCache {
    handles: HashMap<SceneTexture, TextureHandle>,
}

impl TexturePathCache {
    fn get(&self, texture: &SceneTexture) -> Option<TextureHandle> {
        self.handles.get(texture).copied()
    }

    fn insert(&mut self, texture: SceneTexture, handle: TextureHandle) {
        self.handles.insert(texture, handle);
    }

    fn get_or_try_insert_with(
        &mut self,
        texture: &SceneTexture,
        resolve: impl FnOnce() -> Result<TextureHandle>,
    ) -> Result<TextureHandle> {
        if let Some(handle) = self.get(texture) {
            return Ok(handle);
        }

        let handle = resolve()?;
        self.insert(texture.clone(), handle);
        Ok(handle)
    }
}

impl TextureRegistry {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            layout,
            sampler,
            textures: HashMap::new(),
            path_textures: TexturePathCache::default(),
            texture_keys: HashMap::new(),
            next_texture_id: 1,
            upload_count: 0,
        }
    }

    pub(crate) fn resolve_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resolver: &mut AssetResolver,
        texture: &SceneTexture,
    ) -> Result<TextureHandle> {
        self.resolve_with(device, queue, texture, |tex| {
            let resolved = resolver
                .resolve(TextureQuery {
                    tex_path: &tex.tex_path,
                    kind: tex.kind,
                    variant_index: tex.variant_index,
                })
                .with_context(|| format!("resolving scene texture '{}'", tex.tex_path))?;
            if resolved.used_fallback() {
                anyhow::bail!("missing scene texture '{}'", tex.tex_path);
            }
            Ok(resolved.image)
        })
    }

    /// Cache + upload core of `resolve_texture`, parameterized over the asset
    /// fetch. Production uses `resolve_texture` (which closes over an
    /// `AssetResolver`); tests substitute a counted closure to assert the
    /// fetch runs at most once per `SceneTexture`. The fetch closure receives
    /// the original `SceneTexture` so it can apply path/kind/variant logic;
    /// transform is applied here, after the fetch.
    fn resolve_with<F>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &SceneTexture,
        fetch: F,
    ) -> Result<TextureHandle>
    where
        F: FnOnce(&SceneTexture) -> Result<RgbaImage>,
    {
        let mut path_textures = std::mem::take(&mut self.path_textures);
        let result = path_textures.get_or_try_insert_with(texture, || {
            let image = fetch(texture)?;
            Ok(self.register_texture(device, queue, transform_image(image, texture.transform)))
        });
        self.path_textures = path_textures;
        result
    }

    pub(crate) fn register_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        image: RgbaImage,
    ) -> TextureHandle {
        let key = texture_key(&image);
        if let Some(id) = self.texture_keys.get(&key).copied() {
            return id;
        }

        let id = TextureHandle(self.next_texture_id);
        self.next_texture_id += 1;
        let view = create_texture_view(device, queue, &image, "sprite-texture");
        let bind_group = self.create_bind_group_for_view(device, &view, &self.sampler);
        self.texture_keys.insert(key, id);
        self.textures.insert(id, TextureEntry { view, bind_group });
        self.upload_count += 1;
        id
    }

    /// Number of GPU texture uploads performed since registry construction.
    /// Increments only when `register_texture` allocates a new wgpu texture
    /// (cache miss); same-bytes and same-`SceneTexture` calls do not bump it.
    /// Used by the runtime contract test that asserts per-frame scene rebuilds
    /// don't re-upload textures.
    #[cfg(test)]
    pub(crate) fn upload_count(&self) -> usize {
        self.upload_count
    }

    pub(crate) fn bind_group(&self, texture: TextureHandle) -> Option<&wgpu::BindGroup> {
        self.textures.get(&texture).map(|entry| &entry.bind_group)
    }

    pub(crate) fn create_bind_group_for_texture(
        &self,
        device: &wgpu::Device,
        texture: TextureHandle,
        sampler: &wgpu::Sampler,
    ) -> Option<wgpu::BindGroup> {
        let entry = self.textures.get(&texture)?;
        Some(self.create_bind_group_for_view(device, &entry.view, sampler))
    }

    fn create_bind_group_for_view(
        &self,
        device: &wgpu::Device,
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture-bind-group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

fn create_texture_view(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: &RgbaImage,
    label: &str,
) -> wgpu::TextureView {
    let tex_size = wgpu::Extent3d {
        width: image.width(),
        height: image.height(),
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: tex_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        image.as_raw(),
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * image.width()),
            rows_per_image: Some(image.height()),
        },
        tex_size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn texture_key(image: &RgbaImage) -> TextureKey {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    image.as_raw().hash(&mut hasher);
    TextureKey {
        width: image.width(),
        height: image.height(),
        hash: hasher.finish(),
    }
}

fn transform_image(mut image: RgbaImage, transform: SceneTextureTransform) -> RgbaImage {
    match transform {
        SceneTextureTransform::Identity => image,
        SceneTextureTransform::FogLuminanceAlpha => {
            for pixel in image.pixels_mut() {
                let [r, g, b, _] = pixel.0;
                let luminance = (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32).round();
                pixel.0[3] = luminance.clamp(0.0, 255.0) as u8;
            }
            image
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TextureHandle, TexturePathCache, TextureRegistry};
    use crate::defs::GraphicKind;
    use crate::scene::{SceneTexture, SceneTextureTransform};
    use image::RgbaImage;

    #[test]
    fn path_cache_reuses_handle_for_same_scene_texture() {
        let mut cache = TexturePathCache::default();
        let texture = SceneTexture::single("Misc/FogOfWar")
            .with_transform(SceneTextureTransform::FogLuminanceAlpha);
        let mut resolver_calls = 0;

        let first = cache
            .get_or_try_insert_with(&texture, || {
                resolver_calls += 1;
                Ok(TextureHandle(7))
            })
            .unwrap();
        let second = cache
            .get_or_try_insert_with(&texture, || {
                resolver_calls += 1;
                Ok(TextureHandle(8))
            })
            .unwrap();

        assert_eq!(first, TextureHandle(7));
        assert_eq!(second, TextureHandle(7));
        assert_eq!(resolver_calls, 1);
    }

    #[test]
    fn path_cache_keys_include_kind_variant_and_transform() {
        let mut cache = TexturePathCache::default();
        let single = SceneTexture::single("Things/Item/Chunk/ChunkSlag");
        let random = SceneTexture {
            tex_path: "Things/Item/Chunk/ChunkSlag".into(),
            kind: GraphicKind::Random,
            variant_index: 1,
            transform: SceneTextureTransform::Identity,
        };

        cache.insert(single.clone(), TextureHandle(1));
        cache.insert(random.clone(), TextureHandle(2));

        assert_eq!(cache.get(&single), Some(TextureHandle(1)));
        assert_eq!(cache.get(&random), Some(TextureHandle(2)));
    }

    /// Drive the full `resolve_texture` cache+upload path with a counted
    /// fetch closure standing in for `AssetResolver::resolve`. Asserts the
    /// fetch runs at most once per `SceneTexture` and that each successful
    /// fetch corresponds to exactly one `register_texture` upload. This is
    /// the production-shape invariant referenced by
    /// `plans/renderer-engine-boundary/plan-v3.md` Commit 3 done-when:
    /// per-frame scene rebuilds in the live runtime path must not re-upload
    /// textures.
    #[test]
    fn resolve_texture_skips_fetch_and_upload_on_cache_hit() {
        let Some((device, queue)) = headless_device() else {
            eprintln!("skipping: no wgpu adapter available for headless test");
            return;
        };
        let mut registry = TextureRegistry::new(&device);
        let texture = SceneTexture::single("Test/Synthetic");

        let mut fetch_calls = 0;
        let first = registry
            .resolve_with(&device, &queue, &texture, |_tex| {
                fetch_calls += 1;
                Ok(synthetic_rgba(4, 4, [40, 80, 120, 255]))
            })
            .expect("first resolve must succeed");
        assert_eq!(fetch_calls, 1, "first resolve must run the fetch closure");
        assert_eq!(registry.upload_count(), 1);

        let second = registry
            .resolve_with(&device, &queue, &texture, |_tex| {
                fetch_calls += 1;
                // Returning a different image here would never be observable
                // because the path cache short-circuits before the closure
                // runs — that's the property under test.
                Ok(synthetic_rgba(4, 4, [255, 255, 255, 255]))
            })
            .expect("second resolve must succeed via path cache");
        assert_eq!(
            fetch_calls, 1,
            "second resolve of the same SceneTexture must not run fetch"
        );
        assert_eq!(
            registry.upload_count(),
            1,
            "second resolve must not perform a GPU upload"
        );
        assert_eq!(
            first, second,
            "second resolve must return the cached handle"
        );

        // A distinct SceneTexture (different transform) goes to fetch + upload.
        let other = SceneTexture::single("Test/Synthetic")
            .with_transform(SceneTextureTransform::FogLuminanceAlpha);
        let third = registry
            .resolve_with(&device, &queue, &other, |_tex| {
                fetch_calls += 1;
                Ok(synthetic_rgba(4, 4, [10, 20, 30, 255]))
            })
            .expect("third resolve must succeed");
        assert_eq!(fetch_calls, 2);
        assert_eq!(registry.upload_count(), 2);
        assert_ne!(first, third);
    }

    /// Construct a real wgpu device without a window/surface, then assert that
    /// uploading the same image bytes twice does not allocate a second GPU
    /// texture. Companion to `resolve_texture_skips_fetch_and_upload_on_cache_hit`:
    /// that one covers the SceneTexture-keyed cache; this one covers the
    /// content-keyed dedupe inside `register_texture` (e.g. when two distinct
    /// `SceneTexture`s resolve to the same image bytes).
    #[test]
    fn register_texture_dedupes_by_image_bytes() {
        let Some((device, queue)) = headless_device() else {
            eprintln!("skipping: no wgpu adapter available for headless test");
            return;
        };

        let mut registry = TextureRegistry::new(&device);
        let image = synthetic_rgba(8, 8, [200, 50, 75, 255]);
        let identical = synthetic_rgba(8, 8, [200, 50, 75, 255]);

        assert_eq!(registry.upload_count(), 0);
        let first = registry.register_texture(&device, &queue, image);
        assert_eq!(registry.upload_count(), 1);

        let second = registry.register_texture(&device, &queue, identical);
        assert_eq!(first, second, "same bytes must reuse the same handle");
        assert_eq!(
            registry.upload_count(),
            1,
            "second upload of identical bytes must not allocate a new texture"
        );

        // Different bytes should bump the counter — proves the dedupe is
        // content-keyed, not always-cached.
        let other = synthetic_rgba(8, 8, [10, 220, 60, 255]);
        let third = registry.register_texture(&device, &queue, other);
        assert_ne!(first, third);
        assert_eq!(registry.upload_count(), 2);
    }

    fn synthetic_rgba(width: u32, height: u32, pixel: [u8; 4]) -> RgbaImage {
        let mut data = Vec::with_capacity((width * height) as usize * 4);
        for _ in 0..(width * height) {
            data.extend_from_slice(&pixel);
        }
        RgbaImage::from_raw(width, height, data).expect("synthetic image builds")
    }

    fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;
        pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("headless-device-test"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .ok()
    }
}
