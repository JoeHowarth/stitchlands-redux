use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2};
use wgpu::util::DeviceExt;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use super::gpu_context::GpuContext;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    frame_time_seconds: f32,
    screen_width: f32,
    screen_height: f32,
    _pad0: f32,
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

pub(crate) struct CameraState {
    camera: Camera,
    uniform: CameraUniform,
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
    speed: f32,
}

impl CameraState {
    pub(crate) fn new(
        gpu: &GpuContext,
        initial_center: Option<Vec2>,
        initial_zoom: Option<f32>,
    ) -> Self {
        let camera = Camera {
            center: initial_center.unwrap_or(Vec2::new(0.5, 0.5)),
            zoom: initial_zoom.unwrap_or(6.0).max(0.2),
        };
        let uniform = CameraUniform {
            view_proj: camera
                .view_proj(gpu.config.width, gpu.config.height)
                .to_cols_array_2d(),
            frame_time_seconds: 0.0,
            screen_width: gpu.config.width as f32,
            screen_height: gpu.config.height as f32,
            _pad0: 0.0,
        };
        let buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("camera-buffer"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind-group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            camera,
            uniform,
            buffer,
            layout,
            bind_group,
            speed: 0.2,
        }
    }

    pub(crate) fn screen_to_world(&self, gpu: &GpuContext, screen_x: f32, screen_y: f32) -> Vec2 {
        let width = gpu.size.width.max(1) as f32;
        let height = gpu.size.height.max(1) as f32;
        let aspect = width / height;
        let half_h = self.camera.zoom;
        let half_w = half_h * aspect;

        let nx = (screen_x / width).clamp(0.0, 1.0);
        let ny = (screen_y / height).clamp(0.0, 1.0);

        let world_x = self.camera.center.x - half_w + nx * (half_w * 2.0);
        let world_y = self.camera.center.y + half_h - ny * (half_h * 2.0);
        Vec2::new(world_x, world_y)
    }

    pub(crate) fn input(&mut self, gpu: &GpuContext, event: &WindowEvent) -> bool {
        let handled = match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return false;
                }

                match event.physical_key {
                    PhysicalKey::Code(KeyCode::ArrowLeft) | PhysicalKey::Code(KeyCode::KeyA) => {
                        self.camera.center.x -= self.speed;
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) | PhysicalKey::Code(KeyCode::KeyD) => {
                        self.camera.center.x += self.speed;
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) | PhysicalKey::Code(KeyCode::KeyS) => {
                        self.camera.center.y -= self.speed;
                        true
                    }
                    PhysicalKey::Code(KeyCode::ArrowUp) | PhysicalKey::Code(KeyCode::KeyW) => {
                        self.camera.center.y += self.speed;
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyQ) => {
                        self.camera.zoom = (self.camera.zoom * 1.1).min(50.0);
                        true
                    }
                    PhysicalKey::Code(KeyCode::KeyE) => {
                        self.camera.zoom = (self.camera.zoom / 1.1).max(0.2);
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
                true
            }
            _ => false,
        };
        if handled {
            self.update_uniform(gpu);
        }
        handled
    }

    pub(crate) fn update_uniform(&mut self, gpu: &GpuContext) {
        self.uniform.view_proj = self
            .camera
            .view_proj(gpu.config.width, gpu.config.height)
            .to_cols_array_2d();
        self.uniform.screen_width = gpu.config.width as f32;
        self.uniform.screen_height = gpu.config.height as f32;
        gpu.queue
            .write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.uniform));
    }

    pub(crate) fn set_frame_time(&mut self, gpu: &GpuContext, frame_time_seconds: f32) {
        self.uniform.frame_time_seconds = frame_time_seconds;
        gpu.queue
            .write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.uniform));
    }
}
