use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};
use image::RgbaImage;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    pipeline: wgpu::RenderPipeline,
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
    static_sprites: Vec<SpriteInput>,
    dynamic_sprites: Vec<SpriteInput>,
    next_texture_id: u32,
    sprite_batches: Vec<SpriteBatch>,
    camera_speed: f32,
    clear_color: wgpu::Color,
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
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceData {
    world_pos: [f32; 3],
    _pad0: f32,
    size: [f32; 2],
    _pad1: [f32; 2],
    tint: [f32; 4],
}

impl InstanceData {
    fn from_params(params: &SpriteParams) -> Self {
        Self {
            world_pos: [params.world_pos.x, params.world_pos.y, params.world_pos.z],
            _pad0: 0.0,
            size: [params.size.x, params.size.y],
            _pad1: [0.0, 0.0],
            tint: params.tint,
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
                visibility: wgpu::ShaderStages::VERTEX,
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

        let mut out = Self {
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
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
            static_sprites: Vec::new(),
            dynamic_sprites: Vec::new(),
            next_texture_id: 1,
            sprite_batches: Vec::new(),
            camera_speed: 0.2,
            clear_color: wgpu::Color {
                r: options.clear_color[0],
                g: options.clear_color[1],
                b: options.clear_color[2],
                a: options.clear_color[3],
            },
        };
        out.set_static_sprites(sprites)?;
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
        for sprite in &sprites {
            let _ = self.register_texture(sprite.image.clone());
        }
        self.static_sprites = sprites;
        self.rebuild_sprite_batches()
    }

    pub fn set_dynamic_sprites(&mut self, sprites: Vec<SpriteInput>) -> Result<()> {
        for sprite in &sprites {
            let _ = self.register_texture(sprite.image.clone());
        }
        self.dynamic_sprites = sprites;
        self.rebuild_sprite_batches()
    }

    fn rebuild_sprite_batches(&mut self) -> Result<()> {
        let mut grouped: HashMap<TextureId, Vec<(usize, InstanceData)>> = HashMap::new();
        for (index, sprite) in self
            .static_sprites
            .iter()
            .chain(self.dynamic_sprites.iter())
            .enumerate()
        {
            let key = texture_key(&sprite.image);
            let texture_id = self.texture_keys.get(&key).copied().with_context(|| {
                format!(
                    "missing TextureId for sprite texture {}x{}",
                    sprite.image.width(),
                    sprite.image.height()
                )
            })?;
            grouped
                .entry(texture_id)
                .or_default()
                .push((index, InstanceData::from_params(&sprite.params)));
        }

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
            let packed_instances: Vec<InstanceData> =
                instances.into_iter().map(|(_, d)| d).collect();
            let instance_buffer =
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("instance-buffer"),
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
        self.sprite_batches = sprite_batches;
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
        let surface_tex = self.surface.get_current_texture()?;
        let view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("main-encoder"),
            });

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

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            for batch in &self.sprite_batches {
                let texture_bind_group = self
                    .texture_bind_groups
                    .get(&batch.texture_id)
                    .context("missing texture bind group for sprite batch")?;
                pass.set_bind_group(1, texture_bind_group, &[]);
                pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                pass.draw_indexed(0..self.num_indices, 0, 0..batch.instance_count);
            }
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

#[derive(Debug, Clone)]
pub struct SpriteInput {
    pub image: RgbaImage,
    pub params: SpriteParams,
}

#[derive(Debug, Clone)]
pub struct SpriteParams {
    pub world_pos: Vec3,
    pub size: Vec2,
    pub tint: [f32; 4],
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
