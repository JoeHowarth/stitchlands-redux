use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};
use image::RgbaImage;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

/// Format of the offscreen render target written by the water-depth pass
/// and sampled in screen-space by the water-surface pass. R16Float is a good
/// balance: enough precision to avoid visible banding in shore gradients,
/// half the memory of R32Float. Downgrade to R8Unorm only if a target
/// platform lacks R16Float sampling.
const WATER_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
    edge_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    water_depth_pipeline: wgpu::RenderPipeline,
    water_surface_pipeline: wgpu::RenderPipeline,
    noise_bind_group: wgpu::BindGroup,
    water_depth_layout: wgpu::BindGroupLayout,
    water_depth_sampler: wgpu::Sampler,
    water_depth_view: wgpu::TextureView,
    water_depth_bind_group: wgpu::BindGroup,
    water_ramps_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    camera: Camera,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    texture_bind_groups: HashMap<TextureId, wgpu::BindGroup>,
    texture_keys: HashMap<TextureKey, TextureId>,
    texture_images: HashMap<TextureId, RgbaImage>,
    static_instances: Vec<SpriteInstance>,
    dynamic_instances: Vec<SpriteInstance>,
    edge_fans: Vec<EdgeFanInstance>,
    overlay_batches: Vec<ColoredMeshBatch>,
    next_texture_id: u32,
    terrain_sprite_batches: Vec<SpriteBatch>,
    static_sprite_batches: Vec<SpriteBatch>,
    dynamic_sprite_batches: Vec<SpriteBatch>,
    terrain_water_sprite_batches: Vec<SpriteBatch>,
    static_water_sprite_batches: Vec<SpriteBatch>,
    dynamic_water_sprite_batches: Vec<SpriteBatch>,
    edge_sprite_batches: Vec<EdgeSpriteBatch>,
    camera_speed: f32,
    clear_color: wgpu::Color,
    frame_epoch: Instant,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TextureId(u32);

struct SpriteBatch {
    texture_id: TextureId,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
    min_z: f32,
    first_index: usize,
    texture_hash: u64,
}

struct EdgeSpriteBatch {
    texture_id: TextureId,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    min_z: f32,
    first_index: usize,
    texture_hash: u64,
}

type GroupedSpriteInstances = HashMap<TextureId, Vec<(usize, InstanceData)>>;

struct ColoredMeshBatch {
    pass: OverlayPass,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    frame_time_seconds: f32,
    screen_width: f32,
    screen_height: f32,
    _pad0: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceData {
    world_pos: [f32; 3],
    _pad0: f32,
    size: [f32; 2],
    _pad1: [f32; 2],
    tint: [f32; 4],
    uv_rect: [f32; 4],
}

impl InstanceData {
    fn from_params(params: &SpriteParams) -> Self {
        Self {
            world_pos: [params.world_pos.x, params.world_pos.y, params.world_pos.z],
            _pad0: 0.0,
            size: [params.size.x, params.size.y],
            _pad1: [0.0, 0.0],
            tint: params.tint,
            uv_rect: params.uv_rect,
        }
    }

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

struct Camera {
    center: Vec2,
    zoom: f32,
}

impl Camera {
    fn view_proj(&self, width: u32, height: u32) -> Mat4 {
        let aspect = width as f32 / height.max(1) as f32;
        let half_h = self.zoom;
        let half_w = half_h * aspect;
        let left = self.center.x - half_w;
        let right = self.center.x + half_w;
        let bottom = self.center.y - half_h;
        let top = self.center.y + half_h;
        Mat4::orthographic_rh_gl(left, right, bottom, top, -100.0, 100.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct TextureKey {
    width: u32,
    height: u32,
    hash: u64,
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        sprites: Vec<SpriteInput>,
        noise_image: RgbaImage,
        water_assets: crate::water_assets::WaterAssets,
        initial_camera_center: Option<Vec2>,
        options: RendererOptions,
    ) -> Result<Self> {
        if sprites.is_empty() {
            anyhow::bail!("renderer requires at least one sprite");
        }

        if let Some(surface_size) = options.surface_size {
            let _ = window.request_inner_size(surface_size);
        }
        let size = options.surface_size.unwrap_or_else(|| window.inner_size());
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).context("create surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("request adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("request device")?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let alpha_mode = caps.alpha_modes[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

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
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex-buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index-buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let camera = Camera {
            center: initial_camera_center.unwrap_or(Vec2::new(0.5, 0.5)),
            zoom: options.initial_zoom.unwrap_or(6.0).max(0.2),
        };
        let camera_uniform = CameraUniform {
            view_proj: camera
                .view_proj(config.width, config.height)
                .to_cols_array_2d(),
            frame_time_seconds: 0.0,
            screen_width: config.width as f32,
            screen_height: config.height as f32,
            _pad0: 0.0,
        };
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera-buffer"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sprite-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                // Fragment visibility is needed by the water-surface shader,
                // which reads `screen_width`/`screen_height` to compute
                // screen-space UV for sampling the offscreen depth RT.
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&camera_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format,
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

        let noise_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let noise_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("edge-noise-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let noise_bind_group =
            create_noise_bind_group(&device, &queue, &noise_layout, &noise_sampler, &noise_image);

        let edge_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("edge-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("edge_shader.wgsl"))),
        });
        let edge_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("edge-pipeline-layout"),
            bind_group_layouts: &[&camera_layout, &texture_layout, &noise_layout],
            push_constant_ranges: &[],
        });
        let edge_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format,
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

        let overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("colored-overlay-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("colored_overlay.wgsl"))),
        });
        let overlay_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("colored-overlay-pipeline-layout"),
                bind_group_layouts: &[&camera_layout],
                push_constant_ranges: &[],
            });
        let overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                    format,
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

        let water_depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water-depth-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("water_depth.wgsl"))),
        });
        let water_depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("water-depth-pipeline-layout"),
                // slot 1 reuses `noise_layout` so the same noise_bind_group
                // (RoughAlphaAdd) can feed both the edge and water-depth
                // pipelines — it's the same packed asset.
                bind_group_layouts: &[&camera_layout, &noise_layout],
                push_constant_ranges: &[],
            });
        let water_depth_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let water_depth_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-depth-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let (water_depth_view, water_depth_bind_group) = create_water_depth_target(
            &device,
            &water_depth_layout,
            &water_depth_sampler,
            config.width,
            config.height,
        );

        // Surface-pass textures: three ramps + sky reflection + ripple masks
        // + samplers
        // in one bind group. Ramp is picked by `tint.g` (set by
        // `water_shader_params`). Reflection is a global sky overlay sampled
        // in world space with a repeat sampler so it tiles across the map;
        // ripple uses its own repeat sampler and adds small animated
        // distortion in `water_surface.wgsl`.
        // `_AlphaAddTex` is not re-bound here — we reuse `noise_bind_group`
        // at slot 2 (same asset).
        let water_ramps_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let ramp_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-ramp-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let reflection_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("water-reflection-sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let ripple_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
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
            &device,
            &queue,
            "water-shallow-ramp",
            &water_assets.shallow_ramp,
        );
        let deep_view =
            upload_ramp_texture(&device, &queue, "water-deep-ramp", &water_assets.deep_ramp);
        let chest_deep_view = upload_ramp_texture(
            &device,
            &queue,
            "water-chest-deep-ramp",
            &water_assets.chest_deep_ramp,
        );
        let reflection_view = upload_ramp_texture(
            &device,
            &queue,
            "water-reflection",
            &water_assets.reflection,
        );
        let ripple_view =
            upload_ramp_texture(&device, &queue, "water-ripple", &water_assets.ripple);
        let water_ramps_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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

        let water_surface_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water-surface-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("water_surface.wgsl"))),
        });
        let water_surface_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("water-surface-pipeline-layout"),
                bind_group_layouts: &[
                    &camera_layout,
                    &water_depth_layout,
                    &noise_layout,
                    &water_ramps_layout,
                ],
                push_constant_ranges: &[],
            });
        let water_surface_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                        format,
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

        let mut out = Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            edge_pipeline,
            overlay_pipeline,
            water_depth_pipeline,
            water_surface_pipeline,
            noise_bind_group,
            water_depth_layout,
            water_depth_sampler,
            water_depth_view,
            water_depth_bind_group,
            water_ramps_bind_group,
            vertex_buffer,
            index_buffer,
            num_indices: indices.len() as u32,
            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            texture_layout,
            sampler,
            texture_bind_groups: HashMap::new(),
            texture_keys: HashMap::new(),
            texture_images: HashMap::new(),
            static_instances: Vec::new(),
            dynamic_instances: Vec::new(),
            edge_fans: Vec::new(),
            overlay_batches: Vec::new(),
            next_texture_id: 1,
            terrain_sprite_batches: Vec::new(),
            static_sprite_batches: Vec::new(),
            dynamic_sprite_batches: Vec::new(),
            terrain_water_sprite_batches: Vec::new(),
            static_water_sprite_batches: Vec::new(),
            dynamic_water_sprite_batches: Vec::new(),
            edge_sprite_batches: Vec::new(),
            camera_speed: 0.2,
            clear_color: wgpu::Color {
                r: options.clear_color[0],
                g: options.clear_color[1],
                b: options.clear_color[2],
                a: options.clear_color[3],
            },
            frame_epoch: Instant::now(),
        };
        out.set_static_sprites(sprites)?;
        out.set_static_overlays(Vec::new())?;
        out.set_dynamic_sprites(Vec::new())?;
        Ok(out)
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        let (view, bind_group) = create_water_depth_target(
            &self.device,
            &self.water_depth_layout,
            &self.water_depth_sampler,
            self.config.width,
            self.config.height,
        );
        self.water_depth_view = view;
        self.water_depth_bind_group = bind_group;
        self.update_camera_uniform();
    }

    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> Vec2 {
        let width = self.size.width.max(1) as f32;
        let height = self.size.height.max(1) as f32;
        let aspect = width / height;
        let half_h = self.camera.zoom;
        let half_w = half_h * aspect;

        let nx = (screen_x / width).clamp(0.0, 1.0);
        let ny = (screen_y / height).clamp(0.0, 1.0);

        let world_x = self.camera.center.x - half_w + nx * (half_w * 2.0);
        let world_y = self.camera.center.y + half_h - ny * (half_h * 2.0);
        Vec2::new(world_x, world_y)
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return false;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
                        self.camera.center.x -= self.camera_speed;
                        self.update_camera_uniform();
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
                        self.camera.center.x += self.camera_speed;
                        self.update_camera_uniform();
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
                        self.camera.center.y -= self.camera_speed;
                        self.update_camera_uniform();
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
                        self.camera.center.y += self.camera_speed;
                        self.update_camera_uniform();
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyQ) => {
                        self.camera.zoom = (self.camera.zoom * 1.1).min(50.0);
                        self.update_camera_uniform();
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyE) => {
                        self.camera.zoom = (self.camera.zoom / 1.1).max(0.2);
                        self.update_camera_uniform();
                        true
                    }
                    _ => false,
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y * 0.1,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.001,
                };
                self.camera.zoom = (self.camera.zoom * (1.0 - amount)).clamp(0.2, 50.0);
                self.update_camera_uniform();
                true
            }
            _ => false,
        }
    }

    fn update_camera_uniform(&mut self) {
        self.camera_uniform.view_proj = self
            .camera
            .view_proj(self.config.width, self.config.height)
            .to_cols_array_2d();
        self.camera_uniform.screen_width = self.config.width as f32;
        self.camera_uniform.screen_height = self.config.height as f32;
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.camera_uniform),
        );
    }

    pub fn register_texture(&mut self, image: RgbaImage) -> TextureId {
        let key = texture_key(&image);
        if let Some(id) = self.texture_keys.get(&key).copied() {
            return id;
        }

        let id = TextureId(self.next_texture_id);
        self.next_texture_id += 1;
        let bind_group = self.create_texture_bind_group(&image);
        self.texture_keys.insert(key, id);
        self.texture_images.insert(id, image);
        self.texture_bind_groups.insert(id, bind_group);
        id
    }

    pub fn set_static_sprites(&mut self, sprites: Vec<SpriteInput>) -> Result<()> {
        let instances = self.instances_from_sprites(sprites);
        self.set_static_instances(instances)
    }

    pub fn set_static_edge_sprites(&mut self, sprites: Vec<EdgeSpriteInput>) -> Result<()> {
        let fans: Vec<EdgeFanInstance> = sprites
            .into_iter()
            .map(|sprite| EdgeFanInstance {
                texture_id: self.register_texture(sprite.image),
                fan: sprite.fan,
            })
            .collect();
        self.edge_fans = fans;
        self.rebuild_edge_batches()
    }

    pub fn set_static_overlays(&mut self, overlays: Vec<ColoredMeshInput>) -> Result<()> {
        let mut batches = Vec::new();
        for overlay in overlays {
            if overlay.vertices.is_empty() || overlay.indices.is_empty() {
                continue;
            }
            let vertex_count = overlay.vertices.len() as u32;
            if let Some(index) = overlay
                .indices
                .iter()
                .copied()
                .find(|index| *index >= vertex_count)
            {
                anyhow::bail!(
                    "colored overlay index {index} is out of bounds for {vertex_count} vertices"
                );
            }
            let vertex_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("colored-overlay-vertex-buffer"),
                    contents: bytemuck::cast_slice(&overlay.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("colored-overlay-index-buffer"),
                    contents: bytemuck::cast_slice(&overlay.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            batches.push(ColoredMeshBatch {
                pass: overlay.pass,
                vertex_buffer,
                index_buffer,
                index_count: overlay.indices.len() as u32,
            });
        }
        self.overlay_batches = batches;
        Ok(())
    }

    pub fn set_dynamic_sprites(&mut self, sprites: Vec<SpriteInput>) -> Result<()> {
        let instances = self.instances_from_sprites(sprites);
        self.set_dynamic_instances(instances)
    }

    pub fn set_static_instances(&mut self, sprites: Vec<SpriteInstance>) -> Result<()> {
        self.static_instances = sprites;
        self.rebuild_sprite_batches()
    }

    pub fn set_dynamic_instances(&mut self, sprites: Vec<SpriteInstance>) -> Result<()> {
        self.dynamic_instances = sprites;
        self.rebuild_sprite_batches()
    }

    fn instances_from_sprites(&mut self, sprites: Vec<SpriteInput>) -> Vec<SpriteInstance> {
        sprites
            .into_iter()
            .map(|sprite| SpriteInstance {
                texture_id: self.register_texture(sprite.image),
                params: sprite.params,
                is_water: sprite.is_water,
                is_terrain: sprite.is_terrain,
            })
            .collect()
    }

    fn rebuild_sprite_batches(&mut self) -> Result<()> {
        let mut terrain_instances = Vec::new();
        let mut static_instances = Vec::new();
        for sprite in self.static_instances.iter().cloned() {
            if sprite.is_terrain {
                terrain_instances.push(sprite);
            } else {
                static_instances.push(sprite);
            }
        }

        let (terrain_base, terrain_water) = group_sprite_instances(&terrain_instances);
        let (static_base, static_water) = group_sprite_instances(&static_instances);
        let (dynamic_base, dynamic_water) = group_sprite_instances(&self.dynamic_instances);

        self.terrain_sprite_batches =
            pack_sprite_batches(&self.device, terrain_base, "terrain-instance-buffer");
        self.terrain_water_sprite_batches =
            pack_sprite_batches(&self.device, terrain_water, "terrain-water-instance-buffer");
        self.static_sprite_batches =
            pack_sprite_batches(&self.device, static_base, "static-instance-buffer");
        self.static_water_sprite_batches =
            pack_sprite_batches(&self.device, static_water, "static-water-instance-buffer");
        self.dynamic_sprite_batches =
            pack_sprite_batches(&self.device, dynamic_base, "dynamic-instance-buffer");
        self.dynamic_water_sprite_batches =
            pack_sprite_batches(&self.device, dynamic_water, "dynamic-water-instance-buffer");
        Ok(())
    }

    fn rebuild_edge_batches(&mut self) -> Result<()> {
        let mut grouped: HashMap<TextureId, Vec<(usize, EdgeFan)>> = HashMap::new();
        for (index, fan) in self.edge_fans.iter().enumerate() {
            grouped
                .entry(fan.texture_id)
                .or_default()
                .push((index, fan.fan.clone()));
        }

        let mut edge_batches = Vec::with_capacity(grouped.len());
        for (texture_id, mut fans) in grouped {
            fans.sort_by(|a, b| {
                a.1.vertices[0].world_pos[2]
                    .total_cmp(&b.1.vertices[0].world_pos[2])
                    .then_with(|| a.0.cmp(&b.0))
            });
            let min_z = fans
                .iter()
                .map(|(_, f)| f.vertices[0].world_pos[2])
                .fold(f32::INFINITY, f32::min);
            let first_index = fans.first().map(|(idx, _)| *idx).unwrap_or(usize::MAX);

            let mut vertices: Vec<EdgeVertex> = Vec::with_capacity(fans.len() * 9);
            let mut indices: Vec<u32> = Vec::with_capacity(fans.len() * FAN_TRI_INDICES.len());
            for (i, (_, fan)) in fans.iter().enumerate() {
                let base = (i * 9) as u32;
                vertices.extend_from_slice(&fan.vertices);
                for &tri in FAN_TRI_INDICES.iter() {
                    indices.push(base + tri);
                }
            }
            let vertex_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("edge-vertex-buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("edge-index-buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            edge_batches.push(EdgeSpriteBatch {
                texture_id,
                vertex_buffer,
                index_buffer,
                index_count: indices.len() as u32,
                min_z,
                first_index,
                texture_hash: texture_id.0 as u64,
            });
        }

        edge_batches.sort_by(|a, b| {
            a.min_z
                .total_cmp(&b.min_z)
                .then(a.first_index.cmp(&b.first_index))
                .then(a.texture_hash.cmp(&b.texture_hash))
        });
        self.edge_sprite_batches = edge_batches;
        Ok(())
    }

    fn create_texture_bind_group(&self, image: &RgbaImage) -> wgpu::BindGroup {
        let tex_size = wgpu::Extent3d {
            width: image.width(),
            height: image.height(),
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprite-texture"),
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
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
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture-bind-group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    pub fn render(&mut self, screenshot_path: Option<&Path>) -> Result<bool> {
        self.camera_uniform.frame_time_seconds = self.frame_epoch.elapsed().as_secs_f32();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&self.camera_uniform),
        );

        let surface_tex = self.surface.get_current_texture()?;
        let view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("main-encoder"),
            });

        // Water depth pass: writes a single-channel float to the offscreen
        // R16Float RT. Only water sprites participate. The surface pass in
        // the swapchain render reads this RT in screen space to shape the
        // water surface shader output.
        {
            let mut depth_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("water-depth-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.water_depth_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            if !self.terrain_water_sprite_batches.is_empty()
                || !self.static_water_sprite_batches.is_empty()
                || !self.dynamic_water_sprite_batches.is_empty()
            {
                depth_pass.set_pipeline(&self.water_depth_pipeline);
                depth_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                depth_pass.set_bind_group(1, &self.noise_bind_group, &[]);
                depth_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                depth_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                for batch in self
                    .terrain_water_sprite_batches
                    .iter()
                    .chain(self.static_water_sprite_batches.iter())
                    .chain(self.dynamic_water_sprite_batches.iter())
                {
                    depth_pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                    depth_pass.draw_indexed(0..self.num_indices, 0, 0..batch.instance_count);
                }
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            self.draw_overlay_pass(&mut pass, OverlayPass::BeforeWorld);
            self.draw_world_batches(
                &mut pass,
                &self.terrain_sprite_batches,
                &self.terrain_water_sprite_batches,
                Some(&self.edge_sprite_batches),
            )?;
            self.draw_overlay_pass(&mut pass, OverlayPass::AfterTerrain);
            self.draw_world_batches(
                &mut pass,
                &self.static_sprite_batches,
                &self.static_water_sprite_batches,
                None,
            )?;
            self.draw_overlay_pass(&mut pass, OverlayPass::AfterStatic);
            self.draw_world_batches(
                &mut pass,
                &self.dynamic_sprite_batches,
                &self.dynamic_water_sprite_batches,
                None,
            )?;
            self.draw_overlay_pass(&mut pass, OverlayPass::AfterDynamic);
        }

        let readback = if screenshot_path.is_some() {
            Some(self.prepare_screenshot_readback(&mut encoder, &surface_tex.texture))
        } else {
            None
        };

        self.queue.submit(Some(encoder.finish()));
        if let (Some(path), Some(readback)) = (screenshot_path, readback) {
            self.finalize_screenshot(path, readback)?;
        }
        surface_tex.present();
        Ok(screenshot_path.is_some())
    }

    fn prepare_screenshot_readback(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
    ) -> ScreenshotReadback {
        let width = self.config.width;
        let height = self.config.height;
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = padded_bytes_per_row as u64 * height as u64;

        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot-readback"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &readback,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        ScreenshotReadback {
            buffer: readback,
            width,
            height,
            unpadded_bytes_per_row,
            padded_bytes_per_row,
        }
    }

    fn finalize_screenshot(&self, output_path: &Path, readback: ScreenshotReadback) -> Result<()> {
        let slice = readback.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().context("waiting for screenshot map")??;

        let data = slice.get_mapped_range();
        let mut pixels = vec![0u8; (readback.width * readback.height * 4) as usize];
        for y in 0..readback.height as usize {
            let src_start = y * readback.padded_bytes_per_row as usize;
            let src_end = src_start + readback.unpadded_bytes_per_row as usize;
            let dst_start = y * readback.unpadded_bytes_per_row as usize;
            let dst_end = dst_start + readback.unpadded_bytes_per_row as usize;
            pixels[dst_start..dst_end].copy_from_slice(&data[src_start..src_end]);
        }
        drop(data);
        readback.buffer.unmap();

        if self.config.format == wgpu::TextureFormat::Bgra8UnormSrgb
            || self.config.format == wgpu::TextureFormat::Bgra8Unorm
        {
            for chunk in pixels.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
        }

        let image = image::RgbaImage::from_raw(readback.width, readback.height, pixels)
            .context("failed to build screenshot image buffer")?;
        image
            .save(output_path)
            .with_context(|| format!("saving screenshot to {}", output_path.display()))?;
        Ok(())
    }

    pub fn handle_surface_error(&mut self, err: &wgpu::SurfaceError) -> Result<()> {
        match err {
            wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                self.surface.configure(&self.device, &self.config);
                Ok(())
            }
            wgpu::SurfaceError::OutOfMemory => anyhow::bail!("gpu out of memory"),
            wgpu::SurfaceError::Timeout => Ok(()),
        }
    }

    fn draw_overlay_pass<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, overlay_pass: OverlayPass) {
        let mut pipeline_set = false;
        for batch in self
            .overlay_batches
            .iter()
            .filter(|batch| batch.pass == overlay_pass)
        {
            if !pipeline_set {
                pass.set_pipeline(&self.overlay_pipeline);
                pipeline_set = true;
            }
            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
            pass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..1);
        }
    }

    fn draw_world_batches<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        sprite_batches: &'a [SpriteBatch],
        water_sprite_batches: &'a [SpriteBatch],
        edge_sprite_batches: Option<&'a [EdgeSpriteBatch]>,
    ) -> Result<()> {
        #[derive(Clone, Copy)]
        enum DrawKind {
            Base,
            Edge,
            Water,
        }

        let edge_len = edge_sprite_batches
            .map(|batches| batches.len())
            .unwrap_or(0);
        let mut drawables: Vec<(f32, usize, usize, DrawKind)> =
            Vec::with_capacity(sprite_batches.len() + water_sprite_batches.len() + edge_len);
        for (i, batch) in sprite_batches.iter().enumerate() {
            drawables.push((batch.min_z, batch.first_index, i, DrawKind::Base));
        }
        if let Some(edge_batches) = edge_sprite_batches {
            for (i, batch) in edge_batches.iter().enumerate() {
                drawables.push((batch.min_z, batch.first_index, i, DrawKind::Edge));
            }
        }
        for (i, batch) in water_sprite_batches.iter().enumerate() {
            drawables.push((batch.min_z, batch.first_index, i, DrawKind::Water));
        }
        drawables.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let mut current: Option<DrawKind> = None;
        for (_, _, idx, kind) in drawables {
            let need_switch = !matches!(
                (current, kind),
                (Some(DrawKind::Base), DrawKind::Base)
                    | (Some(DrawKind::Edge), DrawKind::Edge)
                    | (Some(DrawKind::Water), DrawKind::Water)
            );
            if need_switch {
                match kind {
                    DrawKind::Base => {
                        pass.set_pipeline(&self.pipeline);
                        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            self.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                    }
                    DrawKind::Edge => {
                        pass.set_pipeline(&self.edge_pipeline);
                        pass.set_bind_group(2, &self.noise_bind_group, &[]);
                    }
                    DrawKind::Water => {
                        pass.set_pipeline(&self.water_surface_pipeline);
                        pass.set_bind_group(1, &self.water_depth_bind_group, &[]);
                        pass.set_bind_group(2, &self.noise_bind_group, &[]);
                        pass.set_bind_group(3, &self.water_ramps_bind_group, &[]);
                        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            self.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                    }
                }
                current = Some(kind);
            }
            match kind {
                DrawKind::Base => {
                    let batch = &sprite_batches[idx];
                    let texture_bind_group = self
                        .texture_bind_groups
                        .get(&batch.texture_id)
                        .context("missing texture bind group for sprite batch")?;
                    pass.set_bind_group(1, texture_bind_group, &[]);
                    pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                    pass.draw_indexed(0..self.num_indices, 0, 0..batch.instance_count);
                }
                DrawKind::Edge => {
                    let edge_batches = edge_sprite_batches.context("missing edge batches")?;
                    let batch = &edge_batches[idx];
                    let texture_bind_group = self
                        .texture_bind_groups
                        .get(&batch.texture_id)
                        .context("missing texture bind group for edge batch")?;
                    pass.set_bind_group(1, texture_bind_group, &[]);
                    pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    pass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..batch.index_count, 0, 0..1);
                }
                DrawKind::Water => {
                    let batch = &water_sprite_batches[idx];
                    pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                    pass.draw_indexed(0..self.num_indices, 0, 0..batch.instance_count);
                }
            }
        }

        Ok(())
    }
}

fn pack_sprite_batches(
    device: &wgpu::Device,
    grouped: HashMap<TextureId, Vec<(usize, InstanceData)>>,
    buffer_label: &'static str,
) -> Vec<SpriteBatch> {
    let mut sprite_batches = Vec::with_capacity(grouped.len());
    for (texture_id, mut instances) in grouped {
        instances.sort_by(|a, b| {
            a.1.world_pos[2]
                .total_cmp(&b.1.world_pos[2])
                .then_with(|| a.0.cmp(&b.0))
        });
        let min_z = instances
            .iter()
            .map(|(_, instance)| instance.world_pos[2])
            .fold(f32::INFINITY, f32::min);
        let first_index = instances.first().map(|(idx, _)| *idx).unwrap_or(usize::MAX);
        let packed_instances: Vec<InstanceData> = instances.into_iter().map(|(_, d)| d).collect();
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(buffer_label),
            contents: bytemuck::cast_slice(&packed_instances),
            usage: wgpu::BufferUsages::VERTEX,
        });
        sprite_batches.push(SpriteBatch {
            texture_id,
            instance_buffer,
            instance_count: packed_instances.len() as u32,
            min_z,
            first_index,
            texture_hash: texture_id.0 as u64,
        });
    }
    sprite_batches.sort_by(|a, b| {
        a.min_z
            .total_cmp(&b.min_z)
            .then(a.first_index.cmp(&b.first_index))
            .then(a.texture_hash.cmp(&b.texture_hash))
    });
    sprite_batches
}

fn group_sprite_instances(
    instances: &[SpriteInstance],
) -> (GroupedSpriteInstances, GroupedSpriteInstances) {
    let mut base_grouped: GroupedSpriteInstances = HashMap::new();
    let mut water_grouped: GroupedSpriteInstances = HashMap::new();
    for (index, sprite) in instances.iter().enumerate() {
        let bucket = if sprite.is_water {
            &mut water_grouped
        } else {
            &mut base_grouped
        };
        bucket
            .entry(sprite.texture_id)
            .or_default()
            .push((index, InstanceData::from_params(&sprite.params)));
    }
    (base_grouped, water_grouped)
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

fn create_water_depth_target(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
) -> (wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("water-depth-target"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WATER_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("water-depth-bind-group"),
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
    });
    (view, bind_group)
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

struct ScreenshotReadback {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OverlayPass {
    BeforeWorld,
    AfterTerrain,
    AfterStatic,
    AfterDynamic,
}

#[derive(Debug, Clone)]
pub struct ColoredMeshInput {
    pub pass: OverlayPass,
    pub vertices: Vec<ColoredVertex>,
    pub indices: Vec<u32>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ColoredVertex {
    pub world_pos: [f32; 3],
    pub color: [f32; 4],
}

impl ColoredVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ColoredVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpriteInput {
    pub image: RgbaImage,
    pub params: SpriteParams,
    /// When true, this sprite is routed through the water depth+surface
    /// pipelines instead of the base pipeline. Today set only for water
    /// terrain cells; in the future any caller that wants a sprite to
    /// participate in water rendering can set it.
    pub is_water: bool,
    pub is_terrain: bool,
}

#[derive(Debug, Clone)]
pub struct SpriteInstance {
    pub texture_id: TextureId,
    pub params: SpriteParams,
    pub is_water: bool,
    pub is_terrain: bool,
}

/// UV sub-rect `(u_min, v_min, u_max, v_max)` covering the full texture.
/// For atlas-indexed sprites, use `linking::atlas_uv_rect` or similar helpers.
pub const FULL_UV_RECT: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Edge-overlay fan submitted to the edge pipeline. The image is the
/// neighbor terrain's base texture; the fan's per-vertex `alpha` drives a
/// radial fade from the matching perimeter verts toward the center.
#[derive(Debug, Clone)]
pub struct EdgeSpriteInput {
    pub image: RgbaImage,
    pub fan: EdgeFan,
}

#[derive(Debug, Clone)]
pub struct EdgeFanInstance {
    pub texture_id: TextureId,
    pub fan: EdgeFan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    FadeRough = 1,
    Water = 2,
}

/// Triangle indices (per fan) for the 8 fan triangles (m, (m+1)%8, 8).
pub const FAN_TRI_INDICES: [u32; 24] = [
    0, 1, 8, 1, 2, 8, 2, 3, 8, 3, 4, 8, 4, 5, 8, 5, 6, 8, 6, 7, 8, 7, 0, 8,
];

/// 9-vertex fan for a single overlay contribution. Vertex order is
/// (0 S mid, 1 SW, 2 W mid, 3 NW, 4 N mid, 5 NE, 6 E mid, 7 SE, 8 center).
/// Center alpha is always 0.
#[derive(Debug, Clone)]
pub struct EdgeFan {
    pub vertices: [EdgeVertex; 9],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct EdgeVertex {
    pub world_pos: [f32; 3],
    pub uv: [f32; 2],
    pub alpha: f32,
    pub noise_seed: [f32; 2],
    pub tint: [f32; 4],
    pub edge_type: u32,
    pub _pad: u32,
}

impl EdgeVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<EdgeVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // world_pos
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // alpha
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                // noise_seed
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // tint
                wgpu::VertexAttribute {
                    offset: 32,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // edge_type
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

/// 1x1 gray fallback noise: `0.5 + r = 1.0` in the shader, so FadeRough/Water
/// edges degrade to a flat fade without the visual variation of the real
/// RoughAlphaAdd texture. Callers should always try to resolve the real asset
/// first; this lets the renderer boot even if it's missing.
pub fn fallback_noise_image() -> RgbaImage {
    RgbaImage::from_raw(1, 1, vec![128, 128, 128, 255]).expect("1x1 image builds")
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

#[derive(Debug, Clone)]
pub struct SpriteParams {
    pub world_pos: Vec3,
    pub size: Vec2,
    pub tint: [f32; 4],
    /// Sub-rect of the texture to sample, as `(u_min, v_min, u_max, v_max)`.
    /// Use `FULL_UV_RECT` for whole-texture sampling.
    pub uv_rect: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
pub struct RendererOptions {
    pub clear_color: [f64; 4],
    pub surface_size: Option<PhysicalSize<u32>>,
    pub initial_zoom: Option<f32>,
}

impl Default for RendererOptions {
    fn default() -> Self {
        Self {
            clear_color: [0.05, 0.08, 0.10, 1.0],
            surface_size: None,
            initial_zoom: None,
        }
    }
}
