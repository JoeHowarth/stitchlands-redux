use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::camera::CameraState;
use super::gpu_context::GpuContext;
use super::pipelines::PipelineSet;
use super::screenshot;
use super::textures::TextureRegistry;
use super::{SunShadowUniform, WATER_DEPTH_FORMAT, validate_textured_mesh_input};
use crate::scene::{
    ColoredMeshInput, EdgeFan, EdgeFanInstance, EdgeSpriteInput, EdgeVertex, FAN_TRI_INDICES,
    Layer, MaterialKind, OverlayBlendMode, SpriteBucket, SpriteParams, SpriteRecord, TextureHandle,
    TexturedMeshInput,
};

pub(crate) struct SpriteBatch {
    texture: TextureHandle,
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) instance_count: u32,
    pub(crate) min_z: f32,
    pub(crate) first_index: usize,
    pub(crate) texture_hash: u64,
}

pub(crate) struct EdgeSpriteBatch {
    texture: TextureHandle,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
    pub(crate) min_z: f32,
    pub(crate) first_index: usize,
    pub(crate) texture_hash: u64,
}

type GroupedSpriteInstances = HashMap<TextureHandle, Vec<(usize, InstanceData)>>;

pub(crate) struct ColoredMeshBatch {
    layer: Layer,
    material: MaterialKind,
    blend_mode: OverlayBlendMode,
    sun_shadow: Option<SunShadowBatch>,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
}

pub(crate) struct TexturedMeshBatch {
    layer: Layer,
    material: MaterialKind,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) index_count: u32,
}

pub(crate) struct SunShadowBatch {
    pub(crate) bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct InstanceData {
    pub(crate) world_pos: [f32; 3],
    _pad0: f32,
    size: [f32; 2],
    _pad1: [f32; 2],
    tint: [f32; 4],
    uv_rect: [f32; 4],
}

impl InstanceData {
    pub(crate) fn from_params(params: &SpriteParams) -> Self {
        Self {
            world_pos: [params.world_pos.x, params.world_pos.y, params.world_pos.z],
            _pad0: 0.0,
            size: [params.size.x, params.size.y],
            _pad1: [0.0, 0.0],
            tint: params.tint,
            uv_rect: params.uv_rect,
        }
    }

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
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

pub(crate) struct FrameRenderer {
    static_instances: Vec<SpriteRecord>,
    dynamic_instances: Vec<SpriteRecord>,
    edge_fans: Vec<EdgeFanInstance>,
    overlay_batches: Vec<ColoredMeshBatch>,
    textured_overlay_batches: Vec<TexturedMeshBatch>,
    terrain_sprite_batches: Vec<SpriteBatch>,
    static_sprite_batches: Vec<SpriteBatch>,
    dynamic_sprite_batches: Vec<SpriteBatch>,
    terrain_water_sprite_batches: Vec<SpriteBatch>,
    static_water_sprite_batches: Vec<SpriteBatch>,
    dynamic_water_sprite_batches: Vec<SpriteBatch>,
    edge_sprite_batches: Vec<EdgeSpriteBatch>,
    water_depth_view: wgpu::TextureView,
    water_depth_bind_group: wgpu::BindGroup,
    clear_color: wgpu::Color,
    frame_epoch: Instant,
}

impl FrameRenderer {
    pub(crate) fn new(gpu: &GpuContext, pipelines: &PipelineSet, clear_color: [f64; 4]) -> Self {
        let (water_depth_view, water_depth_bind_group) = create_water_depth_target(
            &gpu.device,
            &pipelines.water_depth_layout,
            &pipelines.water_depth_sampler,
            gpu.config.width,
            gpu.config.height,
        );

        Self {
            static_instances: Vec::new(),
            dynamic_instances: Vec::new(),
            edge_fans: Vec::new(),
            overlay_batches: Vec::new(),
            textured_overlay_batches: Vec::new(),
            terrain_sprite_batches: Vec::new(),
            static_sprite_batches: Vec::new(),
            dynamic_sprite_batches: Vec::new(),
            terrain_water_sprite_batches: Vec::new(),
            static_water_sprite_batches: Vec::new(),
            dynamic_water_sprite_batches: Vec::new(),
            edge_sprite_batches: Vec::new(),
            water_depth_view,
            water_depth_bind_group,
            clear_color: wgpu::Color {
                r: clear_color[0],
                g: clear_color[1],
                b: clear_color[2],
                a: clear_color[3],
            },
            frame_epoch: Instant::now(),
        }
    }

    pub(crate) fn resize(&mut self, gpu: &GpuContext, pipelines: &PipelineSet) {
        let (view, bind_group) = create_water_depth_target(
            &gpu.device,
            &pipelines.water_depth_layout,
            &pipelines.water_depth_sampler,
            gpu.config.width,
            gpu.config.height,
        );
        self.water_depth_view = view;
        self.water_depth_bind_group = bind_group;
    }

    pub(crate) fn set_static_instances(
        &mut self,
        gpu: &GpuContext,
        sprites: Vec<SpriteRecord>,
    ) -> Result<()> {
        self.static_instances = sprites;
        self.rebuild_sprite_batches(gpu)
    }

    pub(crate) fn set_dynamic_instances(
        &mut self,
        gpu: &GpuContext,
        sprites: Vec<SpriteRecord>,
    ) -> Result<()> {
        self.dynamic_instances = sprites;
        self.rebuild_sprite_batches(gpu)
    }

    pub(crate) fn set_static_edge_sprites(
        &mut self,
        gpu: &GpuContext,
        textures: &mut TextureRegistry,
        sprites: Vec<EdgeSpriteInput>,
    ) -> Result<()> {
        let fans: Vec<EdgeFanInstance> = sprites
            .into_iter()
            .map(|sprite| EdgeFanInstance {
                texture: textures.register_texture(gpu, sprite.image),
                fan: sprite.fan,
                material: sprite.material,
            })
            .collect();
        self.edge_fans = fans;
        self.rebuild_edge_batches(gpu)
    }

    pub(crate) fn set_static_overlays(
        &mut self,
        gpu: &GpuContext,
        pipelines: &PipelineSet,
        overlays: Vec<ColoredMeshInput>,
    ) -> Result<()> {
        let mut batches = Vec::new();
        for overlay in overlays {
            if overlay.vertices.is_empty() || overlay.indices.is_empty() {
                continue;
            }
            match (overlay.blend_mode, overlay.sun_shadow.is_some()) {
                (OverlayBlendMode::SunShadow, false) => {
                    anyhow::bail!("sun shadow overlays require sun shadow parameters");
                }
                (OverlayBlendMode::Alpha | OverlayBlendMode::Multiply, true) => {
                    anyhow::bail!("sun shadow parameters require the sun shadow blend mode");
                }
                _ => {}
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
            let vertex_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("colored-overlay-vertex-buffer"),
                    contents: bytemuck::cast_slice(&overlay.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("colored-overlay-index-buffer"),
                    contents: bytemuck::cast_slice(&overlay.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            let sun_shadow = overlay.sun_shadow.map(|params| {
                let uniform = SunShadowUniform::from_params(params);
                let buffer = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("sun-shadow-uniform-buffer"),
                        contents: bytemuck::bytes_of(&uniform),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sun-shadow-bind-group"),
                    layout: &pipelines.sun_shadow_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                });
                SunShadowBatch { bind_group }
            });
            batches.push(ColoredMeshBatch {
                layer: overlay.layer,
                material: overlay.material,
                blend_mode: overlay.blend_mode,
                sun_shadow,
                vertex_buffer,
                index_buffer,
                index_count: overlay.indices.len() as u32,
            });
        }
        self.overlay_batches = batches;
        Ok(())
    }

    pub(crate) fn set_static_textured_overlays(
        &mut self,
        gpu: &GpuContext,
        textures: &TextureRegistry,
        overlays: Vec<TexturedMeshInput>,
    ) -> Result<()> {
        let mut batches = Vec::new();
        for overlay in overlays {
            if overlay.vertices.is_empty() || overlay.indices.is_empty() {
                continue;
            }
            validate_textured_mesh_input(&overlay)?;
            let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("textured-overlay-sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let bind_group = textures.create_bind_group_with_sampler(
                gpu,
                &overlay.image,
                "textured-overlay-texture",
                &sampler,
            );
            let vertex_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("textured-overlay-vertex-buffer"),
                    contents: bytemuck::cast_slice(&overlay.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("textured-overlay-index-buffer"),
                    contents: bytemuck::cast_slice(&overlay.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            batches.push(TexturedMeshBatch {
                layer: overlay.layer,
                material: overlay.material,
                bind_group,
                vertex_buffer,
                index_buffer,
                index_count: overlay.indices.len() as u32,
            });
        }
        self.textured_overlay_batches = batches;
        Ok(())
    }

    pub(crate) fn render(
        &mut self,
        gpu: &mut GpuContext,
        textures: &TextureRegistry,
        pipelines: &PipelineSet,
        camera: &mut CameraState,
        screenshot_path: Option<&Path>,
    ) -> Result<bool> {
        camera.set_frame_time(gpu, self.frame_epoch.elapsed().as_secs_f32());

        let surface_tex = gpu.surface.get_current_texture()?;
        let view = surface_tex
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = gpu
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
                depth_pass.set_pipeline(&pipelines.water_depth);
                depth_pass.set_bind_group(0, &camera.bind_group, &[]);
                depth_pass.set_bind_group(1, &pipelines.noise_bind_group, &[]);
                depth_pass.set_vertex_buffer(0, pipelines.vertex_buffer.slice(..));
                depth_pass
                    .set_index_buffer(pipelines.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                for batch in self
                    .terrain_water_sprite_batches
                    .iter()
                    .chain(self.static_water_sprite_batches.iter())
                    .chain(self.dynamic_water_sprite_batches.iter())
                {
                    depth_pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                    depth_pass.draw_indexed(0..pipelines.num_indices, 0, 0..batch.instance_count);
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

            pass.set_bind_group(0, &camera.bind_group, &[]);
            self.draw_overlay_layer(&mut pass, pipelines, Layer::BeforeWorld);
            self.draw_textured_overlay_layer(&mut pass, pipelines, Layer::BeforeWorld);
            self.draw_world_batches(
                &mut pass,
                textures,
                pipelines,
                &self.terrain_sprite_batches,
                &self.terrain_water_sprite_batches,
                Some(&self.edge_sprite_batches),
            )?;
            self.draw_overlay_layer(&mut pass, pipelines, Layer::AfterTerrain);
            self.draw_textured_overlay_layer(&mut pass, pipelines, Layer::AfterTerrain);
            self.draw_world_batches(
                &mut pass,
                textures,
                pipelines,
                &self.static_sprite_batches,
                &self.static_water_sprite_batches,
                None,
            )?;
            self.draw_overlay_layer(&mut pass, pipelines, Layer::AfterStatic);
            self.draw_textured_overlay_layer(&mut pass, pipelines, Layer::AfterStatic);
            self.draw_world_batches(
                &mut pass,
                textures,
                pipelines,
                &self.dynamic_sprite_batches,
                &self.dynamic_water_sprite_batches,
                None,
            )?;
            self.draw_overlay_layer(&mut pass, pipelines, Layer::AfterDynamic);
            self.draw_textured_overlay_layer(&mut pass, pipelines, Layer::AfterDynamic);
        }

        let readback = if screenshot_path.is_some() {
            Some(screenshot::prepare_readback(
                gpu,
                &mut encoder,
                &surface_tex.texture,
            ))
        } else {
            None
        };

        gpu.queue.submit(Some(encoder.finish()));
        if let (Some(path), Some(readback)) = (screenshot_path, readback) {
            screenshot::finalize(gpu, path, readback)?;
        }
        surface_tex.present();
        Ok(screenshot_path.is_some())
    }

    fn rebuild_sprite_batches(&mut self, gpu: &GpuContext) -> Result<()> {
        let mut terrain_instances = Vec::new();
        let mut static_instances = Vec::new();
        for sprite in self.static_instances.iter().cloned() {
            match sprite.material.sprite_bucket() {
                SpriteBucket::Terrain | SpriteBucket::TerrainWater => {
                    terrain_instances.push(sprite);
                }
                SpriteBucket::Base => static_instances.push(sprite),
                SpriteBucket::NonSprite => {
                    debug_assert!(false, "non-sprite material submitted as a sprite");
                    static_instances.push(sprite);
                }
            }
        }

        let (terrain_base, terrain_water) = group_sprite_instances(&terrain_instances);
        let (static_base, static_water) = group_sprite_instances(&static_instances);
        let (dynamic_base, dynamic_water) = group_sprite_instances(&self.dynamic_instances);

        self.terrain_sprite_batches =
            pack_sprite_batches(&gpu.device, terrain_base, "terrain-instance-buffer");
        self.terrain_water_sprite_batches =
            pack_sprite_batches(&gpu.device, terrain_water, "terrain-water-instance-buffer");
        self.static_sprite_batches =
            pack_sprite_batches(&gpu.device, static_base, "static-instance-buffer");
        self.static_water_sprite_batches =
            pack_sprite_batches(&gpu.device, static_water, "static-water-instance-buffer");
        self.dynamic_sprite_batches =
            pack_sprite_batches(&gpu.device, dynamic_base, "dynamic-instance-buffer");
        self.dynamic_water_sprite_batches =
            pack_sprite_batches(&gpu.device, dynamic_water, "dynamic-water-instance-buffer");
        Ok(())
    }

    fn rebuild_edge_batches(&mut self, gpu: &GpuContext) -> Result<()> {
        let mut grouped: HashMap<TextureHandle, Vec<(usize, EdgeFan)>> = HashMap::new();
        for (index, fan) in self.edge_fans.iter().enumerate() {
            debug_assert_eq!(fan.material, MaterialKind::TerrainEdge);
            grouped
                .entry(fan.texture)
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
            let vertex_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("edge-vertex-buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("edge-index-buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            edge_batches.push(EdgeSpriteBatch {
                texture: texture_id,
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

    fn draw_overlay_layer<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipelines: &'a PipelineSet,
        layer: Layer,
    ) {
        let mut current_blend_mode = None;
        for batch in self
            .overlay_batches
            .iter()
            .filter(|batch| batch.layer == layer)
        {
            debug_assert!(batch.material.colored_overlay_family());
            if current_blend_mode != Some(batch.blend_mode) {
                pass.set_pipeline(match batch.blend_mode {
                    OverlayBlendMode::Alpha => &pipelines.overlay,
                    OverlayBlendMode::Multiply => &pipelines.overlay_multiply,
                    OverlayBlendMode::SunShadow => &pipelines.sun_shadow,
                });
                current_blend_mode = Some(batch.blend_mode);
            }
            if let Some(sun_shadow) = &batch.sun_shadow {
                pass.set_bind_group(1, &sun_shadow.bind_group, &[]);
            }
            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
            pass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..1);
        }
    }

    fn draw_textured_overlay_layer<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        pipelines: &'a PipelineSet,
        layer: Layer,
    ) {
        let mut pipeline_set = false;
        for batch in self
            .textured_overlay_batches
            .iter()
            .filter(|batch| batch.layer == layer)
        {
            debug_assert!(batch.material.textured_overlay_family());
            if !pipeline_set {
                pass.set_pipeline(&pipelines.textured_overlay);
                pipeline_set = true;
            }
            pass.set_bind_group(1, &batch.bind_group, &[]);
            pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
            pass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..batch.index_count, 0, 0..1);
        }
    }

    fn draw_world_batches<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        textures: &'a TextureRegistry,
        pipelines: &'a PipelineSet,
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
                        pass.set_pipeline(&pipelines.sprite);
                        pass.set_vertex_buffer(0, pipelines.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            pipelines.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                    }
                    DrawKind::Edge => {
                        pass.set_pipeline(&pipelines.edge);
                        pass.set_bind_group(2, &pipelines.noise_bind_group, &[]);
                    }
                    DrawKind::Water => {
                        pass.set_pipeline(&pipelines.water_surface);
                        pass.set_bind_group(1, &self.water_depth_bind_group, &[]);
                        pass.set_bind_group(2, &pipelines.noise_bind_group, &[]);
                        pass.set_bind_group(3, &pipelines.water_ramps_bind_group, &[]);
                        pass.set_vertex_buffer(0, pipelines.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            pipelines.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint16,
                        );
                    }
                }
                current = Some(kind);
            }
            match kind {
                DrawKind::Base => {
                    let batch = &sprite_batches[idx];
                    let texture_bind_group = textures
                        .bind_group(batch.texture)
                        .context("missing texture bind group for sprite batch")?;
                    pass.set_bind_group(1, texture_bind_group, &[]);
                    pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                    pass.draw_indexed(0..pipelines.num_indices, 0, 0..batch.instance_count);
                }
                DrawKind::Edge => {
                    let edge_batches = edge_sprite_batches.context("missing edge batches")?;
                    let batch = &edge_batches[idx];
                    let texture_bind_group = textures
                        .bind_group(batch.texture)
                        .context("missing texture bind group for edge batch")?;
                    pass.set_bind_group(1, texture_bind_group, &[]);
                    pass.set_vertex_buffer(0, batch.vertex_buffer.slice(..));
                    pass.set_index_buffer(batch.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..batch.index_count, 0, 0..1);
                }
                DrawKind::Water => {
                    let batch = &water_sprite_batches[idx];
                    pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                    pass.draw_indexed(0..pipelines.num_indices, 0, 0..batch.instance_count);
                }
            }
        }

        Ok(())
    }
}

fn pack_sprite_batches(
    device: &wgpu::Device,
    grouped: HashMap<TextureHandle, Vec<(usize, InstanceData)>>,
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
            texture: texture_id,
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
    instances: &[SpriteRecord],
) -> (GroupedSpriteInstances, GroupedSpriteInstances) {
    let mut base_grouped: GroupedSpriteInstances = HashMap::new();
    let mut water_grouped: GroupedSpriteInstances = HashMap::new();
    for (index, sprite) in instances.iter().enumerate() {
        let bucket = match sprite.material.sprite_bucket() {
            SpriteBucket::TerrainWater => &mut water_grouped,
            SpriteBucket::Base | SpriteBucket::Terrain => &mut base_grouped,
            SpriteBucket::NonSprite => {
                debug_assert!(false, "non-sprite material submitted as a sprite");
                &mut base_grouped
            }
        };
        bucket
            .entry(sprite.texture)
            .or_default()
            .push((index, InstanceData::from_params(&sprite.params)));
    }
    (base_grouped, water_grouped)
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
