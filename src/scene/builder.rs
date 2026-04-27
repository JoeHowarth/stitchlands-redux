use std::collections::HashMap;

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use log::warn;

use crate::assets::AssetResolver;
use crate::commands::DefSet;
use crate::commands::common::{
    apparel_worn_data_for_facing, build_apparel_tex_path, build_full_apparel_layer_override,
    map_explicit_skip_flags, resolve_directional_tex_path,
};
use crate::commands::linking_sprites::{emit_linked_thing_sprites, emit_terrain_edge_sprites};
use crate::commands::overlays::build_static_overlays;
use crate::defs::{
    ApparelDef, ApparelLayerDef, BeardDefRender, BodyTypeDefRender, HairDefRender,
    HeadTypeDefRender, TerrainDef, ThingDef,
};
use crate::linking::LinkDrawerType;
use crate::pawn::{
    ApparelRenderInput, BeardTypeRenderData, BodyTypeRenderData, HeadTypeRenderData,
    PawnComposeConfig, PawnDrawFlags, PawnFacing, PawnRenderInput, compose_pawn,
};
use crate::water_assets::water_shader_params;
use crate::world::{PawnState, WorldState};

use super::{FULL_UV_RECT, MaterialKind, Scene, SceneSprite, SceneTexture, SpriteParams};

/// Per-pawn render data resolved once at scene initialization (apparel paths,
/// directional textures, body/head/hair/beard defs). Kept stable across frames
/// so per-tick rebuilds only refresh facing and world position, not the whole
/// pawn-graph input.
#[derive(Debug, Clone)]
pub struct PawnVisualProfile {
    pub pawn_id: usize,
    pub base_render_input: PawnRenderInput,
}

/// Cloned copies of every def hashmap, so the runtime can hold defs across
/// frames without lifetime threading. One-time clone at construction; cheap to
/// re-borrow as a `DefSet<'_>` per frame.
#[derive(Debug, Clone)]
pub struct OwnedSceneDefs {
    pub thing_defs: HashMap<String, ThingDef>,
    pub terrain_defs: HashMap<String, TerrainDef>,
    pub apparel_defs: HashMap<String, ApparelDef>,
    pub body_type_defs: HashMap<String, BodyTypeDefRender>,
    pub head_type_defs: HashMap<String, HeadTypeDefRender>,
    pub beard_defs: HashMap<String, BeardDefRender>,
    pub hair_defs: HashMap<String, HairDefRender>,
}

impl OwnedSceneDefs {
    pub fn from_refs(defs: &DefSet<'_>) -> Self {
        Self {
            thing_defs: defs.thing_defs.clone(),
            terrain_defs: defs.terrain_defs.clone(),
            apparel_defs: defs.apparel_defs.clone(),
            body_type_defs: defs.body_type_defs.clone(),
            head_type_defs: defs.head_type_defs.clone(),
            beard_defs: defs.beard_defs.clone(),
            hair_defs: defs.hair_defs.clone(),
        }
    }

    pub fn as_refs(&self) -> DefSet<'_> {
        DefSet {
            thing_defs: &self.thing_defs,
            terrain_defs: &self.terrain_defs,
            apparel_defs: &self.apparel_defs,
            body_type_defs: &self.body_type_defs,
            head_type_defs: &self.head_type_defs,
            beard_defs: &self.beard_defs,
            hair_defs: &self.hair_defs,
        }
    }
}

/// Resolve every pawn currently in the world to a `PawnVisualProfile`. Run
/// once at scene initialization; the resulting map is the contract input to
/// `build_scene` for every subsequent frame.
pub fn compute_pawn_profiles(
    defs: &DefSet<'_>,
    asset_resolver: &mut AssetResolver,
    world: &WorldState,
) -> Result<HashMap<usize, PawnVisualProfile>> {
    let mut out = HashMap::with_capacity(world.pawns().len());
    for pawn in world.pawns() {
        let base_render_input = build_pawn_render_input(defs, asset_resolver, pawn)?;
        out.insert(
            pawn.id,
            PawnVisualProfile {
                pawn_id: pawn.id,
                base_render_input,
            },
        );
    }
    Ok(out)
}

/// Assemble a full `Scene` for the given world snapshot. Pawn profiles must
/// already cover every pawn in the world — call `compute_pawn_profiles` once
/// at scene init and pass the resulting map in unchanged. Pawns missing from
/// the map are an error, not a silent fallback.
///
/// No `AssetResolver` parameter: scene textures are path-backed and resolved
/// to `TextureHandle`s at renderer ingest time. The builder only reads world
/// state and cached pawn profiles.
pub fn build_scene(
    defs: &DefSet<'_>,
    world: &WorldState,
    pawn_profiles: &HashMap<usize, PawnVisualProfile>,
    compose_config: &PawnComposeConfig,
) -> Result<Scene> {
    let mut scene = Scene::default();
    build_terrain_sprites(defs, world, &mut scene)?;
    build_thing_sprites(defs, world, &mut scene)?;
    scene.edge_sprites = emit_terrain_edge_sprites(defs, world)?;

    let static_overlays = build_static_overlays(defs.thing_defs, world)?;
    scene.static_overlays = static_overlays.colored;
    scene.static_textured_overlays = static_overlays.textured;

    build_pawn_sprites(world, pawn_profiles, compose_config, &mut scene)?;
    Ok(scene)
}

fn build_terrain_sprites(defs: &DefSet<'_>, world: &WorldState, scene: &mut Scene) -> Result<()> {
    for z in 0..world.height() {
        for x in 0..world.width() {
            let tile = &world.terrain()[z * world.width() + x];
            let terrain_def = defs
                .terrain_defs
                .get(&tile.terrain_def)
                .with_context(|| format!("missing TerrainDef '{}'", tile.terrain_def))?;
            let water_params = water_shader_params(terrain_def);
            let material = if water_params.is_some() {
                MaterialKind::TerrainWater
            } else {
                MaterialKind::Terrain
            };
            let tint = water_params
                .map(|p| p.to_tint())
                .unwrap_or([1.0, 1.0, 1.0, 1.0]);
            scene.static_sprites.push(SceneSprite {
                def_name: format!("Terrain::{}", terrain_def.def_name),
                texture: SceneTexture::single(terrain_def.texture_path.as_str()),
                params: SpriteParams {
                    world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -1.0),
                    size: Vec2::new(1.0, 1.0),
                    tint,
                    uv_rect: FULL_UV_RECT,
                },
                pawn_id: None,
                node_id: None,
                material,
            });
        }
    }
    Ok(())
}

fn build_thing_sprites(defs: &DefSet<'_>, world: &WorldState, scene: &mut Scene) -> Result<()> {
    let mut things = world.things().to_vec();
    things.sort_by(|a, b| {
        a.cell_z
            .cmp(&b.cell_z)
            .then(a.cell_x.cmp(&b.cell_x))
            .then(a.id.cmp(&b.id))
    });
    for thing in things {
        let thing_def = defs
            .thing_defs
            .get(&thing.def_name)
            .with_context(|| format!("missing ThingDef '{}'", thing.def_name))?;
        if thing_def.graphic_data.link_type != LinkDrawerType::None {
            let linked = emit_linked_thing_sprites(defs, &thing, thing_def, world)?;
            scene.static_sprites.extend(linked);
            continue;
        }
        let draw_offset = thing_def.graphic_data.draw_offset;
        scene.static_sprites.push(SceneSprite {
            def_name: format!("Thing::{}", thing_def.def_name),
            texture: SceneTexture::for_thing(thing_def, thing.id),
            params: SpriteParams {
                world_pos: Vec3::new(
                    thing.cell_x as f32 + 0.5 + draw_offset.x,
                    thing.cell_z as f32 + 0.5 + draw_offset.z,
                    -0.8 + draw_offset.y * 0.01,
                ),
                size: thing_def.graphic_data.draw_size.max(Vec2::splat(1.1)),
                tint: [
                    thing_def.graphic_data.color.r,
                    thing_def.graphic_data.color.g,
                    thing_def.graphic_data.color.b,
                    thing_def.graphic_data.color.a,
                ],
                uv_rect: FULL_UV_RECT,
            },
            pawn_id: None,
            node_id: None,
            material: MaterialKind::Cutout,
        });
    }
    Ok(())
}

fn build_pawn_sprites(
    world: &WorldState,
    pawn_profiles: &HashMap<usize, PawnVisualProfile>,
    compose_config: &PawnComposeConfig,
    scene: &mut Scene,
) -> Result<()> {
    let mut pawns = world.pawns().to_vec();
    pawns.sort_by(|a, b| {
        a.cell_z
            .cmp(&b.cell_z)
            .then(a.cell_x.cmp(&b.cell_x))
            .then(a.id.cmp(&b.id))
    });
    for pawn in pawns {
        let profile = pawn_profiles.get(&pawn.id).with_context(|| {
            format!(
                "pawn {} ('{}') missing from pawn_profiles — \
                 call compute_pawn_profiles whenever the pawn set changes",
                pawn.id, pawn.label
            )
        })?;
        let mut render_input = profile.base_render_input.clone();
        render_input.facing = pawn.facing;
        render_input.world_pos = Vec3::new(pawn.world_pos.x, pawn.world_pos.y, 0.0);

        let composed = compose_pawn(&render_input, compose_config);
        for node in composed.nodes {
            scene.dynamic_sprites.push(SceneSprite {
                def_name: format!("PawnNode::{}", node.id),
                texture: SceneTexture::single(node.tex_path.as_str()),
                params: SpriteParams {
                    world_pos: node.world_pos,
                    size: node.size,
                    tint: node.tint,
                    uv_rect: FULL_UV_RECT,
                },
                pawn_id: Some(pawn.id),
                node_id: Some(node.id),
                material: MaterialKind::Cutout,
            });
        }
    }
    Ok(())
}

fn build_pawn_render_input(
    defs: &DefSet<'_>,
    asset_resolver: &mut AssetResolver,
    pawn: &PawnState,
) -> Result<PawnRenderInput> {
    let body = choose_body_def(defs.body_type_defs, pawn.body.as_deref())?;
    let head = choose_def(
        defs.head_type_defs,
        pawn.head.as_deref(),
        "head",
        |h| &h.def_name,
        |_| true,
    );
    let hair = choose_def(
        defs.hair_defs,
        pawn.hair.as_deref(),
        "hair",
        |h| &h.def_name,
        |_| true,
    );
    let beard = choose_def(
        defs.beard_defs,
        pawn.beard.as_deref(),
        "beard",
        |b| &b.def_name,
        |b| !b.no_graphic && !b.tex_path.is_empty(),
    );

    let facing = pawn.facing;
    let body_directional =
        resolve_directional_tex_path(asset_resolver, &body.body_naked_graphic_path, facing);
    let head_tex_path =
        head.map(|h| resolve_directional_tex_path(asset_resolver, &h.graphic_path, facing).path);
    let hair_tex_path =
        hair.map(|h| resolve_directional_tex_path(asset_resolver, &h.tex_path, facing).path);
    let beard_tex_path =
        beard.map(|b| resolve_directional_tex_path(asset_resolver, &b.tex_path, facing).path);

    let apparel_inputs = build_apparel_inputs(
        defs.apparel_defs,
        &pawn.apparel_defs,
        Some(&body.def_name),
        facing,
        asset_resolver,
    )?;

    Ok(PawnRenderInput {
        label: pawn.label.clone(),
        facing,
        world_pos: Vec3::new(pawn.world_pos.x, pawn.world_pos.y, 0.0),
        body_tex_path: body_directional.path,
        head_tex_path,
        stump_tex_path: None,
        hair_tex_path,
        beard_tex_path,
        body_type: BodyTypeRenderData {
            head_offset: body.head_offset,
        },
        head_type: head
            .map(|v| HeadTypeRenderData {
                narrow: v.narrow,
                beard_offset: v.beard_offset,
                beard_offset_x_east: v.beard_offset_x_east,
            })
            .unwrap_or_default(),
        beard_type: beard
            .map(|v| BeardTypeRenderData {
                offset_narrow_east: v.offset_narrow_east,
                offset_narrow_south: v.offset_narrow_south,
            })
            .unwrap_or_default(),
        tint: [1.0, 1.0, 1.0, 1.0],
        apparel: apparel_inputs,
        draw_flags: PawnDrawFlags::NONE,
    })
}

fn choose_body_def<'a>(
    defs: &'a HashMap<String, BodyTypeDefRender>,
    preferred: Option<&str>,
) -> Result<&'a BodyTypeDefRender> {
    if let Some(preferred) = preferred
        && let Some(body) = defs.get(preferred)
    {
        return Ok(body);
    }
    defs.values()
        .min_by(|a, b| a.def_name.cmp(&b.def_name))
        .context("no BodyTypeDefRender entries are available")
}

fn choose_def<'a, T>(
    defs: &'a HashMap<String, T>,
    preferred: Option<&str>,
    kind: &str,
    key_of: impl Fn(&T) -> &str,
    eligible: impl Fn(&T) -> bool,
) -> Option<&'a T> {
    if let Some(name) = preferred {
        if let Some(value) = defs.get(name) {
            return Some(value);
        }
        warn!("preferred {kind} def '{name}' not found, falling back");
    }
    defs.values()
        .filter(|v| eligible(v))
        .min_by_key(|v| key_of(v))
}

fn build_apparel_inputs(
    defs: &HashMap<String, ApparelDef>,
    apparel_defs: &[String],
    body_def_name: Option<&str>,
    facing: PawnFacing,
    asset_resolver: &mut AssetResolver,
) -> Result<Vec<ApparelRenderInput>> {
    let mut out = Vec::new();
    for def_name in apparel_defs {
        let apparel = defs
            .get(def_name)
            .with_context(|| format!("missing ApparelDef '{}'", def_name))?;

        let render_as_pack = if matches!(apparel.layer, ApparelLayerDef::Belt) {
            apparel.worn_graphic.render_utility_as_pack
        } else {
            false
        };

        let tex_path =
            build_apparel_tex_path(apparel, body_def_name, render_as_pack, asset_resolver);
        let directional = resolve_directional_tex_path(asset_resolver, &tex_path, facing);
        let tex_path = directional.path;
        let worn_data =
            apparel_worn_data_for_facing(apparel, directional.data_facing, body_def_name);
        let (explicit_skip_hair, explicit_skip_beard, has_explicit_skip_flags) =
            map_explicit_skip_flags(&apparel.render_skip_flags);
        let layer_override = build_full_apparel_layer_override(apparel, facing, render_as_pack);
        let anchor_to_head = match apparel.parent_tag_def.as_deref() {
            Some("ApparelHead") => Some(true),
            Some("ApparelBody") => Some(false),
            _ => None,
        };

        out.push(ApparelRenderInput {
            label: apparel.def_name.clone(),
            tex_path,
            layer: apparel.layer.into(),
            explicit_skip_hair,
            explicit_skip_beard,
            has_explicit_skip_flags,
            covers_upper_head: apparel.covers_upper_head,
            covers_full_head: apparel.covers_full_head,
            anchor_to_head,
            pack_offset: worn_data.offset,
            pack_scale: worn_data.scale,
            render_as_pack,
            layer_override,
            tint: [
                apparel.color.r,
                apparel.color.g,
                apparel.color.b,
                apparel.color.a,
            ],
        });
    }
    Ok(out)
}
