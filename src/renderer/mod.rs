use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use image::RgbaImage;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

use crate::assets::AssetResolver;
use crate::scene::{
    ColoredMeshInput, EdgeSpriteInput, SceneTexture, SpriteInput, SpriteRecord, SunShadowParams,
    TextureHandle, TexturedMeshInput,
};

mod camera;
mod frame;
mod gpu_context;
mod pipelines;
mod screenshot;
mod textures;

use camera::CameraState;
use frame::FrameRenderer;
use gpu_context::GpuContext;
use pipelines::PipelineSet;
use textures::TextureRegistry;

/// Format of the offscreen render target written by the water-depth pass
/// and sampled in screen-space by the water-surface pass. R16Float is a good
/// balance: enough precision to avoid visible banding in shore gradients,
/// half the memory of R32Float. Downgrade to R8Unorm only if a target
/// platform lacks R16Float sampling.
const WATER_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

pub struct Renderer {
    gpu: GpuContext,
    textures: TextureRegistry,
    pipelines: PipelineSet,
    frame: FrameRenderer,
    camera: CameraState,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct Vertex {
    pub(crate) pos: [f32; 2],
    pub(crate) uv: [f32; 2],
}

impl Vertex {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
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
pub(crate) struct SunShadowUniform {
    cast_vector: [f32; 4],
    material_color: [f32; 4],
}

impl SunShadowUniform {
    pub(crate) fn from_params(params: SunShadowParams) -> Self {
        Self {
            cast_vector: [
                params.shadow_vector[0],
                params.shadow_vector[1],
                params.shadow_strength,
                0.0,
            ],
            material_color: params.material_color,
        }
    }
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        asset_resolver: &mut AssetResolver,
        sprites: Vec<SpriteInput>,
        noise_image: RgbaImage,
        water_assets: crate::water_assets::WaterAssets,
        initial_camera_center: Option<Vec2>,
        options: RendererOptions,
    ) -> Result<Self> {
        if sprites.is_empty() {
            anyhow::bail!("renderer requires at least one sprite");
        }

        let gpu = GpuContext::new(window, options).await?;
        let textures = TextureRegistry::new(&gpu.device);
        let mut camera = CameraState::new(&gpu, initial_camera_center, options.initial_zoom);
        let pipelines = PipelineSet::build(&gpu, &textures, &camera, &noise_image, &water_assets);
        let frame = FrameRenderer::new(&gpu, &pipelines, options.clear_color);
        camera.update_uniform(&gpu);

        let mut out = Self {
            gpu,
            textures,
            pipelines,
            frame,
            camera,
        };
        out.set_static_sprites(asset_resolver, sprites)?;
        out.set_static_overlays(Vec::new())?;
        out.set_static_textured_overlays(asset_resolver, Vec::new())?;
        out.set_dynamic_sprites(asset_resolver, Vec::new())?;
        Ok(out)
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        if self.gpu.resize(size) {
            self.frame.resize(&self.gpu, &self.pipelines);
            self.camera.update_uniform(&self.gpu);
        }
    }

    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> Vec2 {
        self.camera.screen_to_world(&self.gpu, screen_x, screen_y)
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        self.camera.input(&self.gpu, event)
    }

    pub fn register_texture(&mut self, image: RgbaImage) -> TextureHandle {
        self.textures
            .register_texture(&self.gpu.device, &self.gpu.queue, image)
    }

    pub fn resolve_texture(
        &mut self,
        asset_resolver: &mut AssetResolver,
        texture: &SceneTexture,
    ) -> Result<TextureHandle> {
        self.textures
            .resolve_texture(&self.gpu.device, &self.gpu.queue, asset_resolver, texture)
    }

    pub fn set_static_sprites(
        &mut self,
        asset_resolver: &mut AssetResolver,
        sprites: Vec<SpriteInput>,
    ) -> Result<()> {
        let instances = self.instances_from_sprites(asset_resolver, sprites)?;
        self.set_static_instances(instances)
    }

    pub fn set_static_edge_sprites(
        &mut self,
        asset_resolver: &mut AssetResolver,
        sprites: Vec<EdgeSpriteInput>,
    ) -> Result<()> {
        self.frame
            .set_static_edge_sprites(&self.gpu, asset_resolver, &mut self.textures, sprites)
    }

    pub fn set_static_overlays(&mut self, overlays: Vec<ColoredMeshInput>) -> Result<()> {
        self.frame
            .set_static_overlays(&self.gpu, &self.pipelines, overlays)
    }

    pub fn set_static_textured_overlays(
        &mut self,
        asset_resolver: &mut AssetResolver,
        overlays: Vec<TexturedMeshInput>,
    ) -> Result<()> {
        self.frame.set_static_textured_overlays(
            &self.gpu,
            asset_resolver,
            &mut self.textures,
            overlays,
        )
    }

    pub fn set_dynamic_sprites(
        &mut self,
        asset_resolver: &mut AssetResolver,
        sprites: Vec<SpriteInput>,
    ) -> Result<()> {
        let instances = self.instances_from_sprites(asset_resolver, sprites)?;
        self.set_dynamic_instances(instances)
    }

    pub fn set_static_instances(&mut self, sprites: Vec<SpriteRecord>) -> Result<()> {
        self.frame.set_static_instances(&self.gpu, sprites)
    }

    pub fn set_dynamic_instances(&mut self, sprites: Vec<SpriteRecord>) -> Result<()> {
        self.frame.set_dynamic_instances(&self.gpu, sprites)
    }

    pub fn render(&mut self, screenshot_path: Option<&Path>) -> Result<bool> {
        self.frame.render(
            &mut self.gpu,
            &self.textures,
            &self.pipelines,
            &mut self.camera,
            screenshot_path,
        )
    }

    pub fn handle_surface_error(&mut self, err: &wgpu::SurfaceError) -> Result<()> {
        self.gpu.handle_surface_error(err)
    }

    fn instances_from_sprites(
        &mut self,
        asset_resolver: &mut AssetResolver,
        sprites: Vec<SpriteInput>,
    ) -> Result<Vec<SpriteRecord>> {
        sprites
            .into_iter()
            .map(|sprite| {
                Ok(SpriteRecord {
                    texture: self.textures.resolve_texture(
                        &self.gpu.device,
                        &self.gpu.queue,
                        asset_resolver,
                        &sprite.texture,
                    )?,
                    params: sprite.params,
                    material: sprite.material,
                })
            })
            .collect()
    }
}

fn validate_textured_mesh_input(overlay: &TexturedMeshInput) -> Result<()> {
    let vertex_count = overlay.vertices.len() as u32;
    if let Some(index) = overlay
        .indices
        .iter()
        .copied()
        .find(|index| *index >= vertex_count)
    {
        anyhow::bail!(
            "textured overlay index {index} is out of bounds for {vertex_count} vertices"
        );
    }
    if let Some((vertex_idx, _)) = overlay.vertices.iter().enumerate().find(|(_, vertex)| {
        vertex.world_pos.iter().any(|value| !value.is_finite())
            || vertex.uv.iter().any(|value| !value.is_finite())
            || vertex.color.iter().any(|value| !value.is_finite())
    }) {
        anyhow::bail!("textured overlay vertex {vertex_idx} contains a non-finite value");
    }
    Ok(())
}

/// 1x1 gray fallback noise: `0.5 + r = 1.0` in the shader, so FadeRough/Water
/// edges degrade to a flat fade without the visual variation of the real
/// RoughAlphaAdd texture. Callers should always try to resolve the real asset
/// first; this lets the renderer boot even if it's missing.
pub fn fallback_noise_image() -> RgbaImage {
    RgbaImage::from_raw(1, 1, vec![128, 128, 128, 255]).expect("1x1 image builds")
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
