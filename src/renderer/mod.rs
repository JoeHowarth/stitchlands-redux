use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use image::RgbaImage;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::window::Window;

mod camera;
mod frame;
mod gpu_context;
mod pipelines;
mod screenshot;
mod textures;

use camera::CameraState;
use frame::{FrameRenderer, InstanceData};
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct TextureId(u32);

pub(crate) struct SpriteBatch {
    texture_id: TextureId,
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) instance_count: u32,
    pub(crate) min_z: f32,
    pub(crate) first_index: usize,
    pub(crate) texture_hash: u64,
}

pub(crate) struct EdgeSpriteBatch {
    texture_id: TextureId,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) min_z: f32,
    pub(crate) first_index: usize,
    pub(crate) texture_hash: u64,
}

type GroupedSpriteInstances = HashMap<TextureId, Vec<(usize, InstanceData)>>;

pub(crate) struct ColoredMeshBatch {
    pass: OverlayPass,
    blend_mode: OverlayBlendMode,
    sun_shadow: Option<SunShadowBatch>,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
}

pub(crate) struct TexturedMeshBatch {
    pass: OverlayPass,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
}

pub(crate) struct SunShadowBatch {
    pub(crate) bind_group: wgpu::BindGroup,
}

fn multiply_overlay_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    }
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
        let textures = TextureRegistry::new(&gpu);
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
        out.set_static_sprites(sprites)?;
        out.set_static_overlays(Vec::new())?;
        out.set_static_textured_overlays(Vec::new())?;
        out.set_dynamic_sprites(Vec::new())?;
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

    pub fn register_texture(&mut self, image: RgbaImage) -> TextureId {
        self.textures.register_texture(&self.gpu, image)
    }

    pub fn set_static_sprites(&mut self, sprites: Vec<SpriteInput>) -> Result<()> {
        let instances = self.instances_from_sprites(sprites);
        self.set_static_instances(instances)
    }

    pub fn set_static_edge_sprites(&mut self, sprites: Vec<EdgeSpriteInput>) -> Result<()> {
        self.frame
            .set_static_edge_sprites(&self.gpu, &mut self.textures, sprites)
    }

    pub fn set_static_overlays(&mut self, overlays: Vec<ColoredMeshInput>) -> Result<()> {
        self.frame
            .set_static_overlays(&self.gpu, &self.pipelines, overlays)
    }

    pub fn set_static_textured_overlays(&mut self, overlays: Vec<TexturedMeshInput>) -> Result<()> {
        self.frame
            .set_static_textured_overlays(&self.gpu, &self.textures, overlays)
    }

    pub fn set_dynamic_sprites(&mut self, sprites: Vec<SpriteInput>) -> Result<()> {
        let instances = self.instances_from_sprites(sprites);
        self.set_dynamic_instances(instances)
    }

    pub fn set_static_instances(&mut self, sprites: Vec<SpriteInstance>) -> Result<()> {
        self.frame.set_static_instances(&self.gpu, sprites)
    }

    pub fn set_dynamic_instances(&mut self, sprites: Vec<SpriteInstance>) -> Result<()> {
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

    fn instances_from_sprites(&mut self, sprites: Vec<SpriteInput>) -> Vec<SpriteInstance> {
        sprites
            .into_iter()
            .map(|sprite| SpriteInstance {
                texture_id: self.textures.register_texture(&self.gpu, sprite.image),
                params: sprite.params,
                is_water: sprite.is_water,
                is_terrain: sprite.is_terrain,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OverlayPass {
    BeforeWorld,
    AfterTerrain,
    AfterStatic,
    AfterDynamic,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OverlayBlendMode {
    Alpha,
    Multiply,
    SunShadow,
}

#[derive(Debug, Clone)]
pub struct ColoredMeshInput {
    pub pass: OverlayPass,
    pub blend_mode: OverlayBlendMode,
    pub sun_shadow: Option<SunShadowParams>,
    pub vertices: Vec<ColoredVertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct TexturedMeshInput {
    pub pass: OverlayPass,
    pub image: RgbaImage,
    pub vertices: Vec<TexturedVertex>,
    pub indices: Vec<u32>,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunShadowParams {
    pub shadow_vector: [f32; 2],
    pub shadow_strength: f32,
    pub material_color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ColoredVertex {
    pub world_pos: [f32; 3],
    pub color: [f32; 4],
}

impl ColoredVertex {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
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

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct TexturedVertex {
    pub world_pos: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl TexturedVertex {
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TexturedVertex>() as wgpu::BufferAddress,
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
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 20,
                    shader_location: 2,
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
    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
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
