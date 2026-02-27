use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use log::{info, warn};

use crate::assets::AssetResolver;
use crate::defs::{
    ApparelDef, ApparelLayerDef, ApparelSkipFlagDef, BeardDefRender, BodyTypeDefRender,
    HairDefRender, HeadTypeDefRender, TerrainDef, ThingDef,
};
use crate::pawn::{
    ApparelLayer as ComposeApparelLayer, ApparelRenderInput, BeardTypeRenderData,
    BodyTypeRenderData, HeadTypeRenderData, HediffOverlayInput, OverlayAnchor, PawnComposeConfig,
    PawnDrawFlags, PawnFacing, PawnRenderInput, compose_pawn,
};
use crate::renderer::SpriteParams;
use crate::scene::{
    count_terrain_families, generate_fixture_map, sorted_pawns, sorted_things_by_altitude,
};
use crate::viewer::RenderSprite;

pub(crate) struct FixtureSceneConfig<'a> {
    pub data_dir: &'a Path,
    pub thing_defs: &'a HashMap<String, ThingDef>,
    pub terrain_defs: &'a HashMap<String, TerrainDef>,
    pub apparel_defs: &'a HashMap<String, ApparelDef>,
    pub body_type_defs: &'a HashMap<String, BodyTypeDefRender>,
    pub head_type_defs: &'a HashMap<String, HeadTypeDefRender>,
    pub beard_defs: &'a HashMap<String, BeardDefRender>,
    pub hair_defs: &'a HashMap<String, HairDefRender>,
    pub asset_resolver: &'a mut AssetResolver,
    pub width: usize,
    pub height: usize,
    pub pawn_focus_only: bool,
    pub pawn_audit_mode: bool,
    pub pawn_count: usize,
    pub pawn_fixture_variant: usize,
    pub dump_pawn_trace: Option<PathBuf>,
    pub compose_config: PawnComposeConfig,
    pub strict_missing: bool,
}

pub(crate) fn build_v1_fixture_scene(
    config: FixtureSceneConfig<'_>,
) -> Result<(Vec<RenderSprite>, Vec2)> {
    let FixtureSceneConfig {
        data_dir,
        thing_defs,
        terrain_defs,
        apparel_defs,
        body_type_defs,
        head_type_defs,
        beard_defs,
        hair_defs,
        asset_resolver,
        width,
        height,
        pawn_focus_only,
        pawn_audit_mode,
        pawn_count,
        pawn_fixture_variant,
        dump_pawn_trace,
        compose_config,
        strict_missing,
    } = config;

    let mut terrain_rows: Vec<_> = terrain_defs.values().collect();
    terrain_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));

    let mut chosen_terrain: Vec<(String, image::RgbaImage)> = Vec::new();
    for terrain in terrain_rows {
        let resolved =
            asset_resolver.resolve_texture_path(data_dir, terrain.texture_path.as_str())?;
        if resolved.sprite.used_fallback {
            continue;
        }
        chosen_terrain.push((terrain.def_name.clone(), resolved.sprite.image));
        if chosen_terrain.len() >= 3 {
            break;
        }
    }
    if chosen_terrain.len() < 3 {
        anyhow::bail!("v1 fixture needs at least 3 decodable terrain defs");
    }

    let preferred_things = [
        "Steel",
        "ChunkSlagSteel",
        "Plasteel",
        "WoodLog",
        "ComponentIndustrial",
    ];
    let mut thing_choices = Vec::new();
    for name in preferred_things {
        if let Some(def) = thing_defs.get(name) {
            let resolved = asset_resolver.resolve_thing(data_dir, def)?;
            if resolved.sprite.used_fallback {
                continue;
            }
            thing_choices.push((def.clone(), resolved.sprite.image));
        }
    }
    if thing_choices.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable ThingDef");
    }

    let mut body_rows: Vec<_> = body_type_defs.values().collect();
    body_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_body_choices: Vec<(BodyTypeDefRender, image::RgbaImage)> = Vec::new();
    for body in body_rows {
        let resolved =
            asset_resolver.resolve_texture_path(data_dir, &body.body_naked_graphic_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_body_choices.push((body.clone(), resolved.sprite.image));
    }
    if pawn_body_choices.is_empty() {
        anyhow::bail!("v1 fixture needs at least one decodable pawn body texture");
    }

    let mut head_rows: Vec<_> = head_type_defs.values().collect();
    head_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_head_choices: Vec<(HeadTypeDefRender, image::RgbaImage)> = Vec::new();
    for head in head_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, &head.graphic_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_head_choices.push((head.clone(), resolved.sprite.image));
    }

    let mut hair_rows: Vec<_> = hair_defs.values().collect();
    hair_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_hair_choices: Vec<(HairDefRender, image::RgbaImage)> = Vec::new();
    for hair in hair_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, &hair.tex_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_hair_choices.push((hair.clone(), resolved.sprite.image));
    }

    let mut beard_rows: Vec<_> = beard_defs
        .values()
        .filter(|b| !b.no_graphic && !b.tex_path.is_empty())
        .filter(|b| {
            let key = b.def_name.to_ascii_lowercase();
            let tex = b.tex_path.to_ascii_lowercase();
            !key.contains("anchor") && !tex.contains("beardanchor")
        })
        .collect();
    beard_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut pawn_beard_choices: Vec<(BeardDefRender, image::RgbaImage)> = Vec::new();
    for beard in beard_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, &beard.tex_path)?;
        if resolved.sprite.used_fallback {
            continue;
        }
        pawn_beard_choices.push((beard.clone(), resolved.sprite.image));
    }

    let mut apparel_rows: Vec<_> = apparel_defs.values().collect();
    apparel_rows.sort_by(|a, b| a.def_name.cmp(&b.def_name));
    let mut body_layer_apparel = Vec::new();
    let mut shell_layer_apparel = Vec::new();
    let mut head_layer_apparel = Vec::new();
    for apparel in apparel_rows {
        let resolved = asset_resolver.resolve_texture_path(data_dir, apparel.tex_path.as_str())?;
        if resolved.sprite.used_fallback {
            continue;
        }

        let is_body = matches!(
            apparel.layer,
            ApparelLayerDef::OnSkin | ApparelLayerDef::Middle
        );
        let is_shellish = matches!(
            apparel.layer,
            ApparelLayerDef::Shell | ApparelLayerDef::Belt
        );
        let is_head = matches!(
            apparel.layer,
            ApparelLayerDef::Overhead | ApparelLayerDef::EyeCover
        );

        if is_body {
            body_layer_apparel.push((apparel.clone(), resolved.sprite.image.clone()));
        }
        if is_shellish {
            shell_layer_apparel.push((apparel.clone(), resolved.sprite.image.clone()));
        }
        if is_head {
            head_layer_apparel.push((apparel.clone(), resolved.sprite.image));
        }
    }
    let decodable_apparel_layers = usize::from(!body_layer_apparel.is_empty())
        + usize::from(!shell_layer_apparel.is_empty())
        + usize::from(!head_layer_apparel.is_empty());
    if decodable_apparel_layers == 0 {
        warn!("v1 fixture found no decodable apparel layers; pawns will be unclothed");
    } else if pawn_focus_only && decodable_apparel_layers < 2 {
        anyhow::bail!(
            "pawn fixture requires at least two decodable apparel layers (body + shell/head)"
        );
    }

    let terrain_names = [
        chosen_terrain[0].0.as_str(),
        chosen_terrain[1].0.as_str(),
        chosen_terrain[2].0.as_str(),
    ];
    let thing_names: Vec<String> = if pawn_focus_only || pawn_audit_mode {
        Vec::new()
    } else {
        thing_choices
            .iter()
            .take(20)
            .map(|(def, _)| def.def_name.clone())
            .collect()
    };
    let pawn_tex: Vec<String> = if pawn_focus_only {
        vec![
            pawn_body_choices[pawn_fixture_variant % pawn_body_choices.len()]
                .0
                .body_naked_graphic_path
                .clone(),
        ]
    } else if pawn_audit_mode {
        (0..pawn_count)
            .map(|i| {
                let idx = (pawn_fixture_variant + i) % pawn_body_choices.len();
                pawn_body_choices[idx].0.body_naked_graphic_path.clone()
            })
            .collect()
    } else {
        pawn_body_choices
            .iter()
            .take(6)
            .map(|(body, _)| body.body_naked_graphic_path.clone())
            .collect()
    };
    let mut map = generate_fixture_map(
        width.max(8),
        height.max(8),
        terrain_names,
        &thing_names,
        &pawn_tex,
    );
    if pawn_audit_mode {
        let cols = 5usize;
        let spacing = 4i32;
        let rows = map.pawns.len().div_ceil(cols).max(1);
        let span_x = (cols.saturating_sub(1) as i32) * spacing;
        let span_z = (rows.saturating_sub(1) as i32) * spacing;
        let origin_x = ((map.width as i32 - 1 - span_x) / 2).max(0);
        let origin_z = ((map.height as i32 - 1 - span_z) / 2).max(0);
        for (i, pawn) in map.pawns.iter_mut().enumerate() {
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            pawn.cell_x = (origin_x + col * spacing).min(map.width.saturating_sub(1) as i32);
            pawn.cell_z = (origin_z + row * spacing).min(map.height.saturating_sub(1) as i32);
        }
    }

    let mut terrain_by_name = HashMap::new();
    for (name, image) in chosen_terrain {
        terrain_by_name.insert(name, image);
    }
    let mut thing_by_name = HashMap::new();
    for (def, image) in thing_choices {
        thing_by_name.insert(def.def_name.clone(), (def, image));
    }
    let mut body_by_tex = HashMap::new();
    for (body, _) in &pawn_body_choices {
        body_by_tex.insert(body.body_naked_graphic_path.clone(), body.clone());
    }
    let mut head_by_tex = HashMap::new();
    for (head, _) in &pawn_head_choices {
        head_by_tex.insert(head.graphic_path.clone(), head.clone());
    }
    let mut beard_by_tex = HashMap::new();
    for (beard, _) in &pawn_beard_choices {
        beard_by_tex.insert(beard.tex_path.clone(), beard.clone());
    }
    let mut pawn_layer_by_tex = HashMap::new();
    for (tex_path, image) in pawn_body_choices
        .into_iter()
        .map(|(body, image)| (body.body_naked_graphic_path, image))
        .chain(
            pawn_head_choices
                .into_iter()
                .map(|(head, image)| (head.graphic_path, image)),
        )
        .chain(
            pawn_hair_choices
                .into_iter()
                .map(|(hair, image)| (hair.tex_path, image)),
        )
        .chain(
            pawn_beard_choices
                .into_iter()
                .map(|(beard, image)| (beard.tex_path, image)),
        )
        .chain(
            body_layer_apparel
                .iter()
                .map(|(apparel, image)| (apparel.tex_path.clone(), image.clone())),
        )
        .chain(
            shell_layer_apparel
                .iter()
                .map(|(apparel, image)| (apparel.tex_path.clone(), image.clone())),
        )
        .chain(
            head_layer_apparel
                .iter()
                .map(|(apparel, image)| (apparel.tex_path.clone(), image.clone())),
        )
    {
        pawn_layer_by_tex.insert(tex_path, image);
    }
    let mut head_tex_paths: Vec<String> = pawn_layer_by_tex
        .keys()
        .filter(|path| path.contains("/Heads/"))
        .cloned()
        .collect();
    head_tex_paths.sort();
    let mut hair_tex_paths: Vec<String> = pawn_layer_by_tex
        .keys()
        .filter(|path| path.contains("/Hairs/"))
        .cloned()
        .collect();
    hair_tex_paths.sort();
    let mut beard_tex_paths: Vec<String> = pawn_layer_by_tex
        .keys()
        .filter(|path| path.contains("/Beards/"))
        .cloned()
        .collect();
    beard_tex_paths.sort();

    let mut sprites =
        Vec::with_capacity(map.width * map.height + map.things.len() + map.pawns.len());
    let mut trace_lines = Vec::new();
    trace_lines.push(format!(
        "variant={} map={}x{} pawn_focus_only={} pawn_audit_mode={} pawn_count={}",
        pawn_fixture_variant, map.width, map.height, pawn_focus_only, pawn_audit_mode, pawn_count
    ));
    for z in 0..map.height {
        for x in 0..map.width {
            let name = map.terrain_at(x, z);
            let Some(image) = terrain_by_name.get(name) else {
                continue;
            };
            sprites.push(RenderSprite {
                def_name: format!("Terrain::{name}"),
                image: image.clone(),
                params: SpriteParams {
                    world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -1.0),
                    size: Vec2::new(1.0, 1.0),
                    tint: [1.0, 1.0, 1.0, 1.0],
                },
                used_fallback: false,
                pawn_id: None,
            });
        }
    }

    for thing in sorted_things_by_altitude(&map.things) {
        let Some((def, image)) = thing_by_name.get(&thing.def_name) else {
            continue;
        };
        let draw_offset = def.graphic_data.draw_offset;
        let thing_pos = Vec3::new(
            thing.cell_x as f32 + 0.5 + draw_offset.x,
            thing.cell_z as f32 + 0.5 + draw_offset.z,
            -0.8 + draw_offset.y * 0.01,
        );
        let thing_size = Vec2::new(
            def.graphic_data.draw_size.x.max(1.1),
            def.graphic_data.draw_size.y.max(1.1),
        );
        sprites.push(RenderSprite {
            def_name: format!("Thing::{}", def.def_name),
            image: image.clone(),
            params: SpriteParams {
                world_pos: thing_pos,
                size: thing_size,
                tint: [
                    def.graphic_data.color.r,
                    def.graphic_data.color.g,
                    def.graphic_data.color.b,
                    def.graphic_data.color.a,
                ],
            },
            used_fallback: false,
            pawn_id: None,
        });
    }

    for (pawn_index, pawn) in sorted_pawns(&map.pawns).into_iter().enumerate() {
        let facing = if pawn_focus_only {
            PawnFacing::South
        } else {
            pawn.facing
        };
        let compatible_heads: Vec<&String> = head_tex_paths
            .iter()
            .filter(|path| body_head_compatible(&pawn.tex_path, path))
            .collect();
        let base_head_pool: Vec<&String> = if compatible_heads.is_empty() {
            head_tex_paths.iter().collect()
        } else {
            compatible_heads
        };
        let hair_tex = if hair_tex_paths.is_empty() {
            None
        } else {
            Some(hair_tex_paths[(pawn_fixture_variant / 5) % hair_tex_paths.len()].clone())
        };
        let beard_tex = if beard_tex_paths.is_empty() {
            None
        } else {
            Some(beard_tex_paths[(pawn_fixture_variant / 7) % beard_tex_paths.len()].clone())
        };
        let head_pool: Vec<&String> = if beard_tex.is_some() {
            let male_heads: Vec<&String> = base_head_pool
                .iter()
                .copied()
                .filter(|path| path.to_ascii_lowercase().contains("/male/"))
                .collect();
            if male_heads.is_empty() {
                base_head_pool
            } else {
                male_heads
            }
        } else {
            base_head_pool
        };
        let head_tex = if head_pool.is_empty() {
            None
        } else {
            Some(head_pool[pawn_fixture_variant % head_pool.len()].to_string())
        };
        let body_render = body_by_tex.get(&pawn.tex_path);
        let body_directional =
            resolve_directional_tex_path(asset_resolver, data_dir, &pawn.tex_path, facing);
        let head_render = head_tex
            .as_ref()
            .and_then(|tex| head_by_tex.get(tex))
            .cloned();
        let beard_render = beard_tex
            .as_ref()
            .and_then(|tex| beard_by_tex.get(tex))
            .cloned();
        let selected_apparel_defs = select_fixture_apparel_for_pawn(
            pawn_index,
            pawn_fixture_variant,
            pawn_focus_only,
            &body_layer_apparel,
            &shell_layer_apparel,
            &head_layer_apparel,
        );
        let apparel_inputs: Vec<ApparelRenderInput> = selected_apparel_defs
            .into_iter()
            .map(|(apparel, _)| {
                let layer = apparel.layer.into();
                let render_as_pack = matches!(apparel.layer, ApparelLayerDef::Belt)
                    || apparel.worn_graphic.render_utility_as_pack;
                let mut tex_path = apparel.tex_path.clone();
                if matches!(
                    layer,
                    ComposeApparelLayer::OnSkin
                        | ComposeApparelLayer::Middle
                        | ComposeApparelLayer::Shell
                ) && !render_as_pack
                    && let Some(body) = body_render
                {
                    let suffixed = format!("{}_{}", apparel.tex_path, body.def_name);
                    if let Ok(resolved) = asset_resolver.resolve_texture_path(data_dir, &suffixed)
                        && !resolved.sprite.used_fallback
                    {
                        tex_path = suffixed;
                        let source_label = resolved
                            .sprite
                            .source_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "<unknown>".to_string());
                        pawn_layer_by_tex.insert(tex_path.clone(), resolved.sprite.image);
                        trace_lines.push(format!(
                            "  apparel_path_override {} -> {} ({})",
                            apparel.def_name, tex_path, source_label
                        ));
                    }
                }
                let directional =
                    resolve_directional_tex_path(asset_resolver, data_dir, &tex_path, facing);
                let tex_path = directional.path;
                let worn_data = apparel_worn_data_for_facing(
                    apparel,
                    directional.data_facing,
                    body_render.map(|b| b.def_name.as_str()),
                );
                let (explicit_skip_hair, explicit_skip_beard, has_explicit_skip_flags) =
                    map_explicit_skip_flags(&apparel.render_skip_flags);
                let layer_override = apparel_draw_layer_for_facing(apparel, facing).or_else(|| {
                    if apparel.layer == ApparelLayerDef::Shell
                        && facing == PawnFacing::North
                        && !apparel.shell_rendered_behind_head
                    {
                        Some(88.0)
                    } else if render_as_pack {
                        match facing {
                            PawnFacing::North => Some(93.0),
                            PawnFacing::South => Some(-3.0),
                            PawnFacing::East | PawnFacing::West => None,
                        }
                    } else {
                        None
                    }
                });
                ApparelRenderInput {
                    label: apparel.def_name.clone(),
                    tex_path,
                    layer,
                    explicit_skip_hair,
                    explicit_skip_beard,
                    has_explicit_skip_flags,
                    covers_upper_head: apparel.covers_upper_head,
                    covers_full_head: apparel.covers_full_head,
                    anchor_to_head: match apparel.parent_tag_def.as_deref() {
                        Some("ApparelHead") => Some(true),
                        Some("ApparelBody") => Some(false),
                        _ => None,
                    },
                    draw_offset: worn_data.offset,
                    draw_scale: worn_data.scale,
                    layer_override,
                    draw_size: apparel.draw_size,
                    tint: [
                        apparel.color.r,
                        apparel.color.g,
                        apparel.color.b,
                        apparel.color.a,
                    ],
                }
            })
            .collect();
        let apparel_labels: Vec<String> = apparel_inputs
            .iter()
            .map(|a| format!("{}({:?}: {})", a.label, a.layer, a.tex_path))
            .collect();
        info!(
            "pawn loadout {} facing={:?} body={} head={} hair={} beard={} apparel=[{}]",
            pawn.label,
            facing,
            body_directional.path,
            head_tex.as_deref().unwrap_or("<none>"),
            hair_tex.as_deref().unwrap_or("<none>"),
            beard_tex.as_deref().unwrap_or("<none>"),
            apparel_labels.join(", ")
        );
        let hediff_overlays = if std::env::var_os("STITCHLANDS_ENABLE_DEBUG_HEDIFFS").is_some() {
            vec![
                HediffOverlayInput {
                    label: "TorsoScar".to_string(),
                    tex_path: body_directional.path.clone(),
                    anchor: OverlayAnchor::Body,
                    layer_offset: 1,
                    draw_size: Vec2::new(0.75, 0.75),
                    tint: [1.0, 0.45, 0.45, 0.70],
                    required_body_part_group: Some("Torso".to_string()),
                    visible_facing: Some(vec![PawnFacing::South, PawnFacing::East]),
                },
                HediffOverlayInput {
                    label: "FaceBruise".to_string(),
                    tex_path: head_tex
                        .as_ref()
                        .map(|p| {
                            resolve_directional_tex_path(asset_resolver, data_dir, p, facing).path
                        })
                        .unwrap_or_else(|| body_directional.path.clone()),
                    anchor: OverlayAnchor::Head,
                    layer_offset: 1,
                    draw_size: Vec2::new(0.6, 0.6),
                    tint: [0.75, 0.25, 0.25, 0.60],
                    required_body_part_group: Some("UpperHead".to_string()),
                    visible_facing: None,
                },
            ]
        } else {
            Vec::new()
        };
        let compose_input = PawnRenderInput {
            label: pawn.label.clone(),
            facing,
            world_pos: Vec3::new(pawn.cell_x as f32 + 0.5, pawn.cell_z as f32 + 0.5, 0.0),
            body_tex_path: body_directional.path.clone(),
            head_tex_path: head_tex
                .map(|p| resolve_directional_tex_path(asset_resolver, data_dir, &p, facing).path),
            stump_tex_path: None,
            hair_tex_path: hair_tex
                .map(|p| resolve_directional_tex_path(asset_resolver, data_dir, &p, facing).path),
            beard_tex_path: beard_tex
                .map(|p| resolve_directional_tex_path(asset_resolver, data_dir, &p, facing).path),
            body_size: Vec2::new(1.0, 1.0),
            head_size: head_render
                .as_ref()
                .map(|_| Vec2::new(1.0, 1.0))
                .unwrap_or(Vec2::new(1.0, 1.0)),
            stump_size: Vec2::new(0.8, 0.8),
            hair_size: head_render
                .as_ref()
                .map(|h| h.hair_mesh_size)
                .unwrap_or(Vec2::new(1.0, 1.0)),
            beard_size: head_render
                .as_ref()
                .map(|h| h.beard_mesh_size)
                .unwrap_or(Vec2::new(1.0, 1.0)),
            body_type: BodyTypeRenderData {
                head_offset: body_render
                    .map(|b| b.head_offset)
                    .unwrap_or(Vec2::new(0.0, 0.34)),
                body_size_factor: 1.0,
            },
            head_type: head_render
                .as_ref()
                .map(|h| HeadTypeRenderData {
                    narrow: h.narrow,
                    narrow_crown_horizontal_offset: 0.0,
                    beard_offset: h.beard_offset,
                    beard_offset_x_east: h.beard_offset_x_east,
                })
                .unwrap_or_default(),
            beard_type: beard_render
                .as_ref()
                .map(|b| BeardTypeRenderData {
                    offset_narrow_east: b.offset_narrow_east,
                    offset_narrow_south: b.offset_narrow_south,
                })
                .unwrap_or_default(),
            tint: [1.0, 1.0, 1.0, 1.0],
            apparel: apparel_inputs,
            present_body_part_groups: vec![
                "Torso".to_string(),
                "UpperHead".to_string(),
                "Eyes".to_string(),
            ],
            hediff_overlays,
            draw_flags: PawnDrawFlags::NONE,
        };
        let composed = compose_pawn(&compose_input, &compose_config);
        trace_lines.push(format!(
            "pawn={} facing={:?} body={} head={:?} hair={:?} beard={:?} apparel_count={}",
            pawn.label,
            compose_input.facing,
            compose_input.body_tex_path,
            compose_input.head_tex_path,
            compose_input.hair_tex_path,
            compose_input.beard_tex_path,
            compose_input.apparel.len()
        ));
        for node in &composed.nodes {
            trace_lines.push(format!(
                "  node kind={:?} id={} tex={} pos=({:.3},{:.3},{:.3}) size=({:.3},{:.3}) tint=({:.2},{:.2},{:.2},{:.2})",
                node.kind,
                node.id,
                node.tex_path,
                node.world_pos.x,
                node.world_pos.y,
                node.world_pos.z,
                node.size.x,
                node.size.y,
                node.tint[0],
                node.tint[1],
                node.tint[2],
                node.tint[3]
            ));
        }
        if let Some(head_body_delta) = measure_head_body_delta_y(&composed.nodes) {
            trace_lines.push(format!("  head_body_delta_y={head_body_delta:.3}"));
            if pawn_focus_only && head_body_delta <= 0.0 {
                anyhow::bail!(
                    "upside-down pawn composition detected for variant {pawn_fixture_variant}: head_body_delta_y={head_body_delta:.3}"
                );
            }
        }
        if let Some(violations) = validate_basic_pawn_layering(&composed.nodes)
            && pawn_focus_only
        {
            anyhow::bail!(
                "invalid pawn layering for variant {pawn_fixture_variant}: {}",
                violations.join("; ")
            );
        }

        let body_path = &compose_input.body_tex_path;
        if !pawn_layer_by_tex.contains_key(body_path)
            && let Some(image) = resolve_pawn_texture_image(asset_resolver, data_dir, body_path)
        {
            pawn_layer_by_tex.insert(body_path.clone(), image);
        }
        if !pawn_layer_by_tex.contains_key(body_path) {
            trace_lines.push(format!("  missing_body_image {}", body_path));
            if strict_missing {
                anyhow::bail!("missing pawn body texture: {body_path}");
            }
            continue;
        }

        for node in composed.nodes {
            if !pawn_layer_by_tex.contains_key(&node.tex_path)
                && let Some(image) =
                    resolve_pawn_texture_image(asset_resolver, data_dir, &node.tex_path)
            {
                pawn_layer_by_tex.insert(node.tex_path.clone(), image);
            }
            let Some(image) = pawn_layer_by_tex.get(&node.tex_path) else {
                trace_lines.push(format!("  missing_node_image {}", node.tex_path));
                if strict_missing {
                    anyhow::bail!("missing pawn node texture: {}", node.tex_path);
                }
                continue;
            };
            sprites.push(RenderSprite {
                def_name: format!("Pawn::{}::{:?}::{}", pawn.label, node.kind, node.id),
                image: image.clone(),
                params: SpriteParams {
                    world_pos: node.world_pos,
                    size: node.size,
                    tint: node.tint,
                },
                used_fallback: false,
                pawn_id: None,
            });
        }
    }

    let camera_focus = if pawn_audit_mode {
        Vec2::new(map.width as f32 * 0.5, map.height as f32 * 0.5)
    } else if let Some(thing) = map.things.first() {
        Vec2::new(thing.cell_x as f32 + 0.5, thing.cell_z as f32 + 0.5)
    } else if let Some(pawn) = map.pawns.first() {
        Vec2::new(pawn.cell_x as f32 + 0.5, pawn.cell_z as f32 + 0.5)
    } else {
        Vec2::new(map.width as f32 * 0.5, map.height as f32 * 0.5)
    };

    if pawn_focus_only {
        info!(
            "pawn fixture scene built: map={}x{} terrain_families={} pawns={} drawables={} variant={}",
            map.width,
            map.height,
            count_terrain_families(&map),
            map.pawns.len(),
            sprites.len(),
            pawn_fixture_variant
        );
    } else if pawn_audit_mode {
        info!(
            "pawn audit scene built: map={}x{} terrain_families={} pawns={} drawables={} variant={} (target pawns={})",
            map.width,
            map.height,
            count_terrain_families(&map),
            map.pawns.len(),
            sprites.len(),
            pawn_fixture_variant,
            pawn_count
        );
    } else {
        info!(
            "v1 fixture scene built: map={}x{} terrain_families={} tiles={} things={} pawns={} drawables={} zbands=terrain(-1.0),thing(~-0.8),pawn(~-0.6)",
            map.width,
            map.height,
            count_terrain_families(&map),
            map.width * map.height,
            map.things.len(),
            map.pawns.len(),
            sprites.len()
        );
    }
    if let Some(path) = dump_pawn_trace {
        std::fs::write(&path, trace_lines.join("\n"))
            .with_context(|| format!("writing pawn trace to {}", path.display()))?;
        info!("wrote pawn trace: {}", path.display());
    }
    Ok((sprites, camera_focus))
}

fn measure_head_body_delta_y(nodes: &[crate::pawn::tree::PawnNode]) -> Option<f32> {
    let body_y = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Body))
        .map(|n| n.world_pos.y)?;
    let head_y = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Head))
        .map(|n| n.world_pos.y)
        .or_else(|| {
            nodes
                .iter()
                .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Stump))
                .map(|n| n.world_pos.y)
        })?;
    Some(head_y - body_y)
}

fn validate_basic_pawn_layering(nodes: &[crate::pawn::tree::PawnNode]) -> Option<Vec<String>> {
    let body_z = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Body))
        .map(|n| n.z)?;
    let head_z = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Head))
        .map(|n| n.z)
        .or_else(|| {
            nodes
                .iter()
                .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Stump))
                .map(|n| n.z)
        })?;

    let mut violations = Vec::new();
    if head_z <= body_z {
        violations.push(format!("head_z({head_z:.6}) <= body_z({body_z:.6})"));
    }

    if let Some(hair_z) = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Hair))
        .map(|n| n.z)
        && hair_z <= head_z
    {
        violations.push(format!("hair_z({hair_z:.6}) <= head_z({head_z:.6})"));
    }

    if let Some(beard_z) = nodes
        .iter()
        .find(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Beard))
        .map(|n| n.z)
        && beard_z <= body_z
    {
        violations.push(format!("beard_z({beard_z:.6}) <= body_z({body_z:.6})"));
    }

    if violations.is_empty() {
        None
    } else {
        Some(violations)
    }
}

fn select_fixture_apparel_for_pawn<'a>(
    pawn_index: usize,
    pawn_fixture_variant: usize,
    pawn_focus_only: bool,
    body_layer_apparel: &'a [(ApparelDef, image::RgbaImage)],
    shell_layer_apparel: &'a [(ApparelDef, image::RgbaImage)],
    head_layer_apparel: &'a [(ApparelDef, image::RgbaImage)],
) -> Vec<&'a (ApparelDef, image::RgbaImage)> {
    let seed = pawn_fixture_variant + pawn_index * 17;
    let mut out = Vec::new();

    if !body_layer_apparel.is_empty() && (pawn_focus_only || !pawn_index.is_multiple_of(4)) {
        let idx = seed % body_layer_apparel.len();
        out.push(&body_layer_apparel[idx]);
    }
    if !shell_layer_apparel.is_empty() && (pawn_focus_only || pawn_index.is_multiple_of(2)) {
        let idx = (seed / 2).max(1) % shell_layer_apparel.len();
        out.push(&shell_layer_apparel[idx]);
    }
    if !head_layer_apparel.is_empty() && (pawn_focus_only || !pawn_index.is_multiple_of(3)) {
        let idx = (seed / 3).max(1) % head_layer_apparel.len();
        out.push(&head_layer_apparel[idx]);
    }

    if out.is_empty() {
        if !shell_layer_apparel.is_empty() {
            out.push(&shell_layer_apparel[seed % shell_layer_apparel.len()]);
        } else if !body_layer_apparel.is_empty() {
            out.push(&body_layer_apparel[seed % body_layer_apparel.len()]);
        } else if !head_layer_apparel.is_empty() {
            out.push(&head_layer_apparel[seed % head_layer_apparel.len()]);
        }
    }

    out
}

fn map_explicit_skip_flags(flags: &Option<Vec<ApparelSkipFlagDef>>) -> (bool, bool, bool) {
    let Some(flags) = flags else {
        return (false, false, false);
    };
    let mut skip_hair = false;
    let mut skip_beard = false;
    for flag in flags {
        match flag {
            ApparelSkipFlagDef::Hair => skip_hair = true,
            ApparelSkipFlagDef::Beard => skip_beard = true,
            ApparelSkipFlagDef::None | ApparelSkipFlagDef::Eyes => {}
        }
    }
    (skip_hair, skip_beard, true)
}

fn apparel_draw_layer_for_facing(apparel: &ApparelDef, facing: PawnFacing) -> Option<f32> {
    match facing {
        PawnFacing::North => apparel.draw_data.north_layer,
        PawnFacing::East => apparel.draw_data.east_layer,
        PawnFacing::South => apparel.draw_data.south_layer,
        PawnFacing::West => apparel.draw_data.west_layer,
    }
}

fn apparel_worn_data_for_facing(
    apparel: &ApparelDef,
    facing: PawnFacing,
    body_type: Option<&str>,
) -> crate::defs::ApparelWornDirectionDef {
    let body_key = body_type.map(|s| s.to_ascii_lowercase());
    let (mut out, directional_overrides) = match facing {
        PawnFacing::North => (
            apparel.worn_graphic.north,
            &apparel.worn_graphic.north_body_overrides,
        ),
        PawnFacing::East => (
            apparel.worn_graphic.east,
            &apparel.worn_graphic.east_body_overrides,
        ),
        PawnFacing::South => (
            apparel.worn_graphic.south,
            &apparel.worn_graphic.south_body_overrides,
        ),
        PawnFacing::West => (
            apparel.worn_graphic.west,
            &apparel.worn_graphic.west_body_overrides,
        ),
    };
    if let Some(body_key) = body_key {
        if let Some(global) = apparel.worn_graphic.global_body_overrides.get(&body_key) {
            if let Some(offset) = global.offset {
                out.offset = offset;
            }
            if let Some(scale) = global.scale {
                out.scale = scale;
            }
        }
        if let Some(local) = directional_overrides.get(&body_key) {
            if let Some(offset) = local.offset {
                out.offset = offset;
            }
            if let Some(scale) = local.scale {
                out.scale = scale;
            }
        }
    }
    out
}

struct DirectionalTexturePath {
    path: String,
    data_facing: PawnFacing,
}

fn resolve_directional_tex_path(
    asset_resolver: &mut AssetResolver,
    data_dir: &Path,
    path: &str,
    facing: PawnFacing,
) -> DirectionalTexturePath {
    if path.ends_with("_north")
        || path.ends_with("_south")
        || path.ends_with("_east")
        || path.ends_with("_west")
    {
        return DirectionalTexturePath {
            path: path.to_string(),
            data_facing: facing,
        };
    }

    let candidates: &[(PawnFacing, &str)] = match facing {
        PawnFacing::North => &[(PawnFacing::North, "_north")],
        PawnFacing::South => &[(PawnFacing::South, "_south")],
        PawnFacing::East => &[
            (PawnFacing::East, "_east"),
            (PawnFacing::West, "_west"),
        ],
        PawnFacing::West => &[
            (PawnFacing::West, "_west"),
            (PawnFacing::East, "_east"),
        ],
    };

    for (data_facing, suffix) in candidates {
        let candidate = format!("{path}{suffix}");
        if let Ok(resolved) = asset_resolver.resolve_texture_path(data_dir, &candidate)
            && !resolved.sprite.used_fallback
        {
            return DirectionalTexturePath {
                path: candidate,
                data_facing: *data_facing,
            };
        }
    }

    let (data_facing, suffix) = candidates[0];
    DirectionalTexturePath {
        path: format!("{path}{suffix}"),
        data_facing,
    }
}

fn strip_directional_suffix(path: &str) -> Option<&str> {
    path.strip_suffix("_north")
        .or_else(|| path.strip_suffix("_south"))
        .or_else(|| path.strip_suffix("_east"))
        .or_else(|| path.strip_suffix("_west"))
}

fn resolve_pawn_texture_image(
    asset_resolver: &mut AssetResolver,
    data_dir: &Path,
    path: &str,
) -> Option<image::RgbaImage> {
    if let Ok(resolved) = asset_resolver.resolve_texture_path(data_dir, path)
        && !resolved.sprite.used_fallback
    {
        return Some(resolved.sprite.image);
    }
    if let Some(base_path) = strip_directional_suffix(path)
        && let Ok(resolved) = asset_resolver.resolve_texture_path(data_dir, base_path)
        && !resolved.sprite.used_fallback
    {
        return Some(resolved.sprite.image);
    }
    None
}

fn body_head_compatible(body_tex: &str, head_tex: &str) -> bool {
    let body_lower = body_tex.to_ascii_lowercase();
    let head_lower = head_tex.to_ascii_lowercase();
    if body_lower.contains("female") {
        return head_lower.contains("female");
    }
    if body_lower.contains("male") {
        return head_lower.contains("male");
    }
    true
}

pub(crate) fn make_missing_def_message(
    thingdef: &str,
    defs: &std::collections::HashMap<String, ThingDef>,
) -> String {
    let mut suggestions: Vec<&str> = defs
        .keys()
        .filter_map(|name| {
            if name.eq_ignore_ascii_case(thingdef) {
                Some(name.as_str())
            } else {
                let name_lower = name.to_lowercase();
                let query_lower = thingdef.to_lowercase();
                if name_lower.contains(&query_lower) || query_lower.contains(&name_lower) {
                    Some(name.as_str())
                } else {
                    None
                }
            }
        })
        .take(5)
        .collect();
    suggestions.sort_unstable();

    if suggestions.is_empty() {
        format!("thingdef '{thingdef}' not found")
    } else {
        format!(
            "thingdef '{thingdef}' not found. Close matches: {}",
            suggestions.join(", ")
        )
    }
}
