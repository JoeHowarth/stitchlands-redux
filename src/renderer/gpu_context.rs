use std::sync::Arc;

use anyhow::{Context, Result};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use super::RendererOptions;

pub(crate) struct GpuContext {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) size: PhysicalSize<u32>,
}

impl GpuContext {
    pub(crate) async fn new(window: Arc<Window>, options: RendererOptions) -> Result<Self> {
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

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size,
        })
    }

    pub(crate) fn resize(&mut self, size: PhysicalSize<u32>) -> bool {
        if size.width == 0 || size.height == 0 {
            return false;
        }
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
        true
    }

    pub(crate) fn handle_surface_error(&mut self, err: &wgpu::SurfaceError) -> Result<()> {
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
