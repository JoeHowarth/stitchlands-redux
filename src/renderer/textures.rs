use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use anyhow::{Context, Result};
use image::RgbaImage;

use super::gpu_context::GpuContext;
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
}

impl TextureRegistry {
    pub(crate) fn new(gpu: &GpuContext) -> Self {
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        }
    }

    pub(crate) fn resolve_texture(
        &mut self,
        gpu: &GpuContext,
        resolver: &mut AssetResolver,
        texture: &SceneTexture,
    ) -> Result<TextureHandle> {
        if let Some(handle) = self.path_textures.get(texture) {
            return Ok(handle);
        }

        let resolved = resolver
            .resolve(TextureQuery {
                tex_path: &texture.tex_path,
                kind: texture.kind,
                variant_index: texture.variant_index,
            })
            .with_context(|| format!("resolving scene texture '{}'", texture.tex_path))?;
        if resolved.used_fallback() {
            anyhow::bail!("missing scene texture '{}'", texture.tex_path);
        }

        let handle = self.register_texture(gpu, transform_image(resolved.image, texture.transform));
        self.path_textures.insert(texture.clone(), handle);
        Ok(handle)
    }

    pub(crate) fn register_texture(&mut self, gpu: &GpuContext, image: RgbaImage) -> TextureHandle {
        let key = texture_key(&image);
        if let Some(id) = self.texture_keys.get(&key).copied() {
            return id;
        }

        let id = TextureHandle(self.next_texture_id);
        self.next_texture_id += 1;
        let view = self.create_texture_view(gpu, &image, "sprite-texture");
        let bind_group = self.create_bind_group_for_view(&gpu.device, &view, &self.sampler);
        self.texture_keys.insert(key, id);
        self.textures.insert(id, TextureEntry { view, bind_group });
        id
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

    fn create_texture_view(
        &self,
        gpu: &GpuContext,
        image: &RgbaImage,
        label: &str,
    ) -> wgpu::TextureView {
        let tex_size = wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        };
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
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
    use super::{TextureHandle, TexturePathCache};
    use crate::defs::GraphicKind;
    use crate::scene::{SceneTexture, SceneTextureTransform};

    #[test]
    fn path_cache_reuses_handle_for_same_scene_texture() {
        let mut cache = TexturePathCache::default();
        let texture = SceneTexture::single("Misc/FogOfWar")
            .with_transform(SceneTextureTransform::FogLuminanceAlpha);

        cache.insert(texture.clone(), TextureHandle(7));

        assert_eq!(cache.get(&texture), Some(TextureHandle(7)));
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
}
