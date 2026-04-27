use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use image::RgbaImage;

use super::TextureId;
use super::gpu_context::GpuContext;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct TextureKey {
    width: u32,
    height: u32,
    hash: u64,
}

pub(crate) struct TextureRegistry {
    pub(crate) layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    bind_groups: HashMap<TextureId, wgpu::BindGroup>,
    texture_keys: HashMap<TextureKey, TextureId>,
    next_texture_id: u32,
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
            bind_groups: HashMap::new(),
            texture_keys: HashMap::new(),
            next_texture_id: 1,
        }
    }

    pub(crate) fn register_texture(&mut self, gpu: &GpuContext, image: RgbaImage) -> TextureId {
        let key = texture_key(&image);
        if let Some(id) = self.texture_keys.get(&key).copied() {
            return id;
        }

        let id = TextureId(self.next_texture_id);
        self.next_texture_id += 1;
        let bind_group = self.create_bind_group(gpu, &image);
        self.texture_keys.insert(key, id);
        self.bind_groups.insert(id, bind_group);
        id
    }

    pub(crate) fn bind_group(&self, texture_id: TextureId) -> Option<&wgpu::BindGroup> {
        self.bind_groups.get(&texture_id)
    }

    pub(crate) fn create_bind_group(&self, gpu: &GpuContext, image: &RgbaImage) -> wgpu::BindGroup {
        self.create_bind_group_with_sampler(gpu, image, "sprite-texture", &self.sampler)
    }

    pub(crate) fn create_bind_group_with_sampler(
        &self,
        gpu: &GpuContext,
        image: &RgbaImage,
        label: &str,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
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
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture-bind-group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
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
