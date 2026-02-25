use std::borrow::Cow;
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
    sprites: Vec<SpriteDraw>,
    camera_speed: f32,
}

struct SpriteDraw {
    sprite_bind_group: wgpu::BindGroup,
    texture_bind_group: wgpu::BindGroup,
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
struct SpriteUniform {
    world_pos: [f32; 3],
    _pad0: f32,
    size: [f32; 2],
    _pad1: [f32; 2],
    tint: [f32; 4],
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

impl Renderer {
    pub async fn new(window: Arc<Window>, sprites: Vec<SpriteInput>) -> Result<Self> {
        if sprites.is_empty() {
            anyhow::bail!("renderer requires at least one sprite");
        }

        let size = window.inner_size();
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
            center: Vec2::new(0.5, 0.5),
            zoom: 6.0,
        };
        let camera_uniform = CameraUniform {
            view_proj: camera.view_proj(config.width, config.height).to_cols_array_2d(),
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
        let sprite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sprite-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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
        let mut sprite_draws = Vec::with_capacity(sprites.len());
        for sprite in &sprites {
            let sprite_uniform = SpriteUniform {
                world_pos: [
                    sprite.params.world_pos.x,
                    sprite.params.world_pos.y,
                    sprite.params.world_pos.z,
                ],
                _pad0: 0.0,
                size: [sprite.params.size.x, sprite.params.size.y],
                _pad1: [0.0, 0.0],
                tint: sprite.params.tint,
            };
            let sprite_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sprite-buffer"),
                contents: bytemuck::bytes_of(&sprite_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let tex_size = wgpu::Extent3d {
                width: sprite.image.width(),
                height: sprite.image.height(),
                depth_or_array_layers: 1,
            };
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sprite-texture"),
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
                sprite.image.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * sprite.image.width()),
                    rows_per_image: Some(sprite.image.height()),
                },
                tex_size,
            );
            let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let sprite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sprite-bind-group"),
                layout: &sprite_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sprite_buffer.as_entire_binding(),
                }],
            });
            let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("texture-bind-group"),
                layout: &texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });

            sprite_draws.push(SpriteDraw {
                sprite_bind_group,
                texture_bind_group,
            });
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline-layout"),
            bind_group_layouts: &[&camera_layout, &sprite_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sprite-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
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

        Ok(Self {
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
            sprites: sprite_draws,
            camera_speed: 0.2,
        })
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
                    PhysicalKey::Code(KeyCode::ArrowRight)
                    | PhysicalKey::Code(KeyCode::KeyD) => {
                        self.camera.center.x += self.camera_speed;
                        self.update_camera_uniform();
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown)
                    | PhysicalKey::Code(KeyCode::KeyS) => {
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
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&self.camera_uniform));
    }

    pub fn render(&mut self) -> Result<()> {
        let surface_tex = self.surface.get_current_texture()?;
        let view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder =
            self.device
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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.08,
                            b: 0.1,
                            a: 1.0,
                        }),
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
            for sprite in &self.sprites {
                pass.set_bind_group(1, &sprite.sprite_bind_group, &[]);
                pass.set_bind_group(2, &sprite.texture_bind_group, &[]);
                pass.draw_indexed(0..self.num_indices, 0, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        surface_tex.present();
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
