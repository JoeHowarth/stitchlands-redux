use std::borrow::Cow;

use image::RgbaImage;
use wgpu::util::DeviceExt;

use super::camera::CameraState;
use super::frame::InstanceData;
use super::gpu_context::GpuContext;
use super::textures::TextureRegistry;
use super::{ColoredVertex, EdgeVertex, Vertex, WATER_DEPTH_FORMAT, multiply_overlay_blend_state};

pub(crate) struct PipelineSet {
    pub(crate) sprite: wgpu::RenderPipeline,
    pub(crate) edge: wgpu::RenderPipeline,
    pub(crate) overlay: wgpu::RenderPipeline,
    pub(crate) overlay_multiply: wgpu::RenderPipeline,
    pub(crate) textured_overlay: wgpu::RenderPipeline,
    pub(crate) sun_shadow: wgpu::RenderPipeline,
    pub(crate) sun_shadow_layout: wgpu::BindGroupLayout,
    pub(crate) water_depth: wgpu::RenderPipeline,
    pub(crate) water_surface: wgpu::RenderPipeline,
    pub(crate) noise_bind_group: wgpu::BindGroup,
    pub(crate) water_depth_layout: wgpu::BindGroupLayout,
    pub(crate) water_depth_sampler: wgpu::Sampler,
    pub(crate) water_ramps_bind_group: wgpu::BindGroup,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) num_indices: u32,
}

impl PipelineSet {
    pub(crate) fn build(
        gpu: &GpuContext,
        textures: &TextureRegistry,
        camera: &CameraState,
        noise_image: &RgbaImage,
        water_assets: &crate::water_assets::WaterAssets,
    ) -> Self {
        let vertices = [
            Vertex {
                pos: [-0.5, -0.5],
                uv: [0.0, 1.0],
            },
            Vertex {
                pos: [0.5, -0.5],
                uv: [1.0, 1.0],
            },
            Vertex {
                pos: [0.5, 0.5],
                uv: [1.0, 0.0],
            },
            Vertex {
                pos: [-0.5, 0.5],
                uv: [0.0, 0.0],
            },
        ];
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let vertex_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex-buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("index-buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sprite-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shader.wgsl"))),
            });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("pipeline-layout"),
                bind_group_layouts: &[&camera.layout, &textures.layout],
                push_constant_ranges: &[],
            });

        let sprite = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sprite-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc(), InstanceData::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let noise_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("edge-noise-layout"),
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
        let noise_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("edge-noise-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let noise_bind_group = create_noise_bind_group(
            &gpu.device,
            &gpu.queue,
            &noise_layout,
            &noise_sampler,
            noise_image,
        );

        let edge_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("edge-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../edge_shader.wgsl"
                ))),
            });
        let edge_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("edge-pipeline-layout"),
                    bind_group_layouts: &[&camera.layout, &textures.layout, &noise_layout],
                    push_constant_ranges: &[],
                });
        let edge = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("edge-pipeline"),
                layout: Some(&edge_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &edge_shader,
                    entry_point: "vs_main",
                    buffers: &[EdgeVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &edge_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let overlay_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("colored-overlay-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../colored_overlay.wgsl"
                ))),
            });
        let textured_overlay_shader =
            gpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("textured-overlay-shader"),
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                        "../textured_overlay.wgsl"
                    ))),
                });
        let overlay_multiply_shader =
            gpu.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("colored-overlay-multiply-shader"),
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                        "../colored_overlay_multiply.wgsl"
                    ))),
                });
        let sun_shadow_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("sun-shadow-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../sun_shadow.wgsl"))),
            });
        let sun_shadow_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("sun-shadow-layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        let overlay_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("colored-overlay-pipeline-layout"),
                    bind_group_layouts: &[&camera.layout],
                    push_constant_ranges: &[],
                });
        let textured_overlay_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("textured-overlay-pipeline-layout"),
                    bind_group_layouts: &[&camera.layout, &textures.layout],
                    push_constant_ranges: &[],
                });
        let sun_shadow_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("sun-shadow-pipeline-layout"),
                    bind_group_layouts: &[&camera.layout, &sun_shadow_layout],
                    push_constant_ranges: &[],
                });
        let overlay = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("colored-overlay-pipeline"),
                layout: Some(&overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &overlay_shader,
                    entry_point: "vs_main",
                    buffers: &[ColoredVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &overlay_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });
        let textured_overlay = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("textured-overlay-pipeline"),
                layout: Some(&textured_overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &textured_overlay_shader,
                    entry_point: "vs_main",
                    buffers: &[super::TexturedVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &textured_overlay_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });
        let overlay_multiply = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("colored-overlay-multiply-pipeline"),
                layout: Some(&overlay_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &overlay_multiply_shader,
                    entry_point: "vs_main",
                    buffers: &[ColoredVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &overlay_multiply_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(multiply_overlay_blend_state()),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });
        let sun_shadow = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sun-shadow-pipeline"),
                layout: Some(&sun_shadow_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &sun_shadow_shader,
                    entry_point: "vs_main",
                    buffers: &[ColoredVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &sun_shadow_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(multiply_overlay_blend_state()),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let water_depth_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("water-depth-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../water_depth.wgsl"
                ))),
            });
        let water_depth_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("water-depth-pipeline-layout"),
                    // slot 1 reuses `noise_layout` so the same noise_bind_group
                    // (RoughAlphaAdd) can feed both the edge and water-depth
                    // pipelines — it's the same packed asset.
                    bind_group_layouts: &[&camera.layout, &noise_layout],
                    push_constant_ranges: &[],
                });
        let water_depth = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("water-depth-pipeline"),
                layout: Some(&water_depth_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &water_depth_shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc(), InstanceData::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &water_depth_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: WATER_DEPTH_FORMAT,
                        // The depth pass "paints" values into the RT with
                        // straight replace semantics. Blending would confuse
                        // downstream sampling — if two water cells overlap in
                        // screen space (they don't today, but in principle),
                        // take the last one written.
                        blend: None,
                        write_mask: wgpu::ColorWrites::RED,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        let water_depth_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("water-depth-layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                // R16Float is a non-filterable float sample type
                                // under wgpu default limits; declare accordingly.
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                    ],
                });
        let water_depth_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-depth-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Surface-pass textures: three ramps + sky reflection + ripple masks
        // + samplers in one bind group. Ramp is picked by `tint.g` (set by
        // `water_shader_params`). Reflection is a global sky overlay sampled
        // in world space with a repeat sampler so it tiles across the map;
        // ripple uses its own repeat sampler and adds small animated
        // distortion in `water_surface.wgsl`.
        // `_AlphaAddTex` is not re-bound here — we reuse `noise_bind_group`
        // at slot 2 (same asset).
        let water_ramps_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("water-ramps-layout"),
                    entries: &[
                        ramp_texture_entry(0),
                        ramp_texture_entry(1),
                        ramp_texture_entry(2),
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        ramp_texture_entry(4),
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        ramp_texture_entry(6),
                        wgpu::BindGroupLayoutEntry {
                            binding: 7,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });
        let ramp_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-ramp-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let reflection_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-reflection-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let ripple_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-ripple-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shallow_view = upload_ramp_texture(
            &gpu.device,
            &gpu.queue,
            "water-shallow-ramp",
            &water_assets.shallow_ramp,
        );
        let deep_view = upload_ramp_texture(
            &gpu.device,
            &gpu.queue,
            "water-deep-ramp",
            &water_assets.deep_ramp,
        );
        let chest_deep_view = upload_ramp_texture(
            &gpu.device,
            &gpu.queue,
            "water-chest-deep-ramp",
            &water_assets.chest_deep_ramp,
        );
        let reflection_view = upload_ramp_texture(
            &gpu.device,
            &gpu.queue,
            "water-reflection",
            &water_assets.reflection,
        );
        let ripple_view = upload_ramp_texture(
            &gpu.device,
            &gpu.queue,
            "water-ripple",
            &water_assets.ripple,
        );
        let water_ramps_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("water-ramps-bind-group"),
            layout: &water_ramps_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shallow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&deep_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&chest_deep_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&ramp_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&reflection_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&reflection_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&ripple_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&ripple_sampler),
                },
            ],
        });

        let water_surface_shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("water-surface-shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../water_surface.wgsl"
                ))),
            });
        let water_surface_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("water-surface-pipeline-layout"),
                    bind_group_layouts: &[
                        &camera.layout,
                        &water_depth_layout,
                        &noise_layout,
                        &water_ramps_layout,
                    ],
                    push_constant_ranges: &[],
                });
        let water_surface = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("water-surface-pipeline"),
                layout: Some(&water_surface_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &water_surface_shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc(), InstanceData::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &water_surface_shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: gpu.config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Self {
            sprite,
            edge,
            overlay,
            overlay_multiply,
            textured_overlay,
            sun_shadow,
            sun_shadow_layout,
            water_depth,
            water_surface,
            noise_bind_group,
            water_depth_layout,
            water_depth_sampler,
            water_ramps_bind_group,
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
        }
    }
}

fn ramp_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn upload_ramp_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &'static str,
    image: &RgbaImage,
) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: image.width(),
        height: image.height(),
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Ramps are sampled as color, so keep sRGB so the gradient reads
        // correctly when sampled linearly.
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
        size,
    );
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

fn create_noise_bind_group(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    image: &RgbaImage,
) -> wgpu::BindGroup {
    let tex_size = wgpu::Extent3d {
        width: image.width(),
        height: image.height(),
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("edge-noise-texture"),
        size: tex_size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Linear (non-sRGB) — the noise is treated as a mask value, not a color.
        format: wgpu::TextureFormat::Rgba8Unorm,
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
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("edge-noise-bind-group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
