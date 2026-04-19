use std::path::Path;

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use log::warn;

use crate::assets::AssetResolver;
use crate::cell::Cell;
use crate::defs::ThingDef;
use crate::linking::{LinkDrawerType, LinkFlags, atlas_uv_rect, corner_filler_positions, link_index};
use crate::renderer::SpriteParams;
use crate::viewer::RenderSprite;
use crate::world::{
    DEPTH_WALL, DEPTH_WALL_CORNER, ThingState, WorldState, cardinal_neighbors, diagonal_neighbors,
};

use super::common::DefSet;

/// RimWorld's `Graphic_LinkedCornerFiller` samples a single point (0.5, 0.6)
/// from the atlas for every corner-quad vertex — degenerate UVs give a
/// solid-interior fill pulled from the fully-connected body.
pub const CORNER_FILL_UV_RECT: [f32; 4] = [0.5, 0.6, 0.5, 0.6];

const CORNER_FILL_XY_OFFSET: f32 = 0.25;
const CORNER_FILL_SIZE: f32 = 0.5;
/// Matches `Graphic_LinkedCornerFiller.ShiftUp`: a 0.09-cell nudge north that
/// RimWorld applies to every corner filler for perspective bias.
const CORNER_FILL_Z_SHIFT: f32 = 0.09;

pub fn emit_linked_thing_sprites(
    data_dir: &Path,
    asset_resolver: &mut AssetResolver,
    defs: &DefSet<'_>,
    thing: &ThingState,
    thing_def: &ThingDef,
    world: &WorldState,
    strict_missing: bool,
) -> Result<Vec<RenderSprite>> {
    let atlas_path = linked_atlas_path(thing_def);
    let resolved = asset_resolver
        .resolve_texture_path(data_dir, &atlas_path)
        .with_context(|| {
            format!(
                "resolving linked atlas '{}' for '{}'",
                atlas_path, thing_def.def_name
            )
        })?;
    if strict_missing && resolved.sprite.used_fallback {
        anyhow::bail!(
            "missing linked atlas '{}' for '{}'",
            atlas_path,
            thing_def.def_name
        );
    }

    let cell = Cell::new(thing.cell_x, thing.cell_z);
    let self_flags = thing_def.graphic_data.link_flags;
    let neighbors = cardinal_neighbor_info(defs, world, cell, self_flags);
    let index = link_index(self_flags, neighbors.flags);
    let uv_rect = atlas_uv_rect(index);

    let effective_type = effective_link_type(thing_def);
    let tint = [
        thing_def.graphic_data.color.r,
        thing_def.graphic_data.color.g,
        thing_def.graphic_data.color.b,
        thing_def.graphic_data.color.a,
    ];

    let mut sprites = Vec::with_capacity(1);
    sprites.push(RenderSprite {
        def_name: format!("Thing::{}", thing_def.def_name),
        image: resolved.sprite.image.clone(),
        params: SpriteParams {
            world_pos: Vec3::new(cell.x as f32 + 0.5, cell.z as f32 + 0.5, DEPTH_WALL),
            size: Vec2::new(1.0, 1.0),
            tint,
            uv_rect,
        },
        used_fallback: resolved.sprite.used_fallback,
        pawn_id: None,
    });

    if effective_type == LinkDrawerType::CornerFiller {
        let diag_links = diagonal_links(defs, world, cell, self_flags);
        let corners = corner_filler_positions(neighbors.links, diag_links);
        // Order matches DIAGONAL_OFFSETS: NE, SE, SW, NW.
        let corner_offsets = [
            (
                CORNER_FILL_XY_OFFSET,
                CORNER_FILL_XY_OFFSET + CORNER_FILL_Z_SHIFT,
            ),
            (
                CORNER_FILL_XY_OFFSET,
                -CORNER_FILL_XY_OFFSET + CORNER_FILL_Z_SHIFT,
            ),
            (
                -CORNER_FILL_XY_OFFSET,
                -CORNER_FILL_XY_OFFSET + CORNER_FILL_Z_SHIFT,
            ),
            (
                -CORNER_FILL_XY_OFFSET,
                CORNER_FILL_XY_OFFSET + CORNER_FILL_Z_SHIFT,
            ),
        ];
        for (i, &emit) in corners.iter().enumerate() {
            if !emit {
                continue;
            }
            let (ox, oz) = corner_offsets[i];
            sprites.push(RenderSprite {
                def_name: format!("Thing::{}", thing_def.def_name),
                image: resolved.sprite.image.clone(),
                params: SpriteParams {
                    world_pos: Vec3::new(
                        cell.x as f32 + 0.5 + ox,
                        cell.z as f32 + 0.5 + oz,
                        DEPTH_WALL_CORNER,
                    ),
                    size: Vec2::new(CORNER_FILL_SIZE, CORNER_FILL_SIZE),
                    tint,
                    uv_rect: CORNER_FILL_UV_RECT,
                },
                used_fallback: resolved.sprite.used_fallback,
                pawn_id: None,
            });
        }
    }

    Ok(sprites)
}

/// Derive the atlas texture path for a linked thing def.
///
/// - `Graphic_Appearances` (walls): real atlas lives in a sub-folder named
///   after the base, with `_Atlas_{Appearance}` suffix. We pick `Bricks`
///   because it's the generic non-stuff-specific variant available on every
///   stuffable wall.
/// - Everything else (rocks, conduits, barricades, fences): texPath already
///   ends in `_Atlas` — use it as-is.
fn linked_atlas_path(thing_def: &ThingDef) -> String {
    let tex_path = thing_def.graphic_data.tex_path.as_str();
    let basename = tex_path.rsplit('/').next().unwrap_or(tex_path);
    if basename.contains("_Atlas") {
        return tex_path.to_string();
    }
    match thing_def.graphic_data.graphic_class.as_deref() {
        Some("Graphic_Appearances") => format!("{tex_path}/{basename}_Atlas_Bricks"),
        _ => tex_path.to_string(),
    }
}

fn effective_link_type(thing_def: &ThingDef) -> LinkDrawerType {
    match thing_def.graphic_data.link_type {
        LinkDrawerType::None | LinkDrawerType::Basic => LinkDrawerType::Basic,
        LinkDrawerType::CornerFiller => LinkDrawerType::CornerFiller,
        other => {
            warn!(
                "linkType {:?} on '{}' not implemented; rendering as Basic",
                other, thing_def.def_name
            );
            LinkDrawerType::Basic
        }
    }
}

struct CardinalNeighbors {
    flags: [Option<LinkFlags>; 4],
    links: [bool; 4],
}

fn cardinal_neighbor_info(
    defs: &DefSet<'_>,
    world: &WorldState,
    cell: Cell,
    self_flags: LinkFlags,
) -> CardinalNeighbors {
    let mut flags = [Some(LinkFlags::EMPTY); 4];
    let mut links = [false; 4];
    for (i, neighbor) in cardinal_neighbors(cell).iter().enumerate() {
        if !world.cell_in_bounds(*neighbor) {
            flags[i] = None;
            links[i] = self_flags.contains(LinkFlags::MAP_EDGE);
            continue;
        }
        let merged = merged_link_flags_at(defs, world, *neighbor);
        flags[i] = Some(merged);
        links[i] = merged.intersects(self_flags);
    }
    CardinalNeighbors { flags, links }
}

fn diagonal_links(
    defs: &DefSet<'_>,
    world: &WorldState,
    cell: Cell,
    self_flags: LinkFlags,
) -> [bool; 4] {
    let mut links = [false; 4];
    for (i, diag) in diagonal_neighbors(cell).iter().enumerate() {
        if !world.cell_in_bounds(*diag) {
            links[i] = self_flags.contains(LinkFlags::MAP_EDGE);
            continue;
        }
        links[i] = merged_link_flags_at(defs, world, *diag).intersects(self_flags);
    }
    links
}

fn merged_link_flags_at(defs: &DefSet<'_>, world: &WorldState, cell: Cell) -> LinkFlags {
    let mut merged = LinkFlags::EMPTY;
    for &thing_idx in world.things_at(cell) {
        let other = &world.things()[thing_idx];
        if let Some(other_def) = defs.thing_defs.get(&other.def_name) {
            merged |= other_def.graphic_data.link_flags;
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{GraphicData, RgbaColor};

    fn make_def(def_name: &str, tex_path: &str, graphic_class: Option<&str>) -> ThingDef {
        ThingDef {
            def_name: def_name.to_string(),
            graphic_data: GraphicData {
                tex_path: tex_path.to_string(),
                graphic_class: graphic_class.map(str::to_string),
                color: RgbaColor::WHITE,
                draw_size: Vec2::ONE,
                draw_offset: Vec3::ZERO,
                link_type: LinkDrawerType::CornerFiller,
                link_flags: LinkFlags::WALL,
            },
        }
    }

    #[test]
    fn appearances_path_goes_to_subfolder_bricks() {
        let def = make_def(
            "Wall",
            "Things/Building/Linked/Wall",
            Some("Graphic_Appearances"),
        );
        assert_eq!(
            linked_atlas_path(&def),
            "Things/Building/Linked/Wall/Wall_Atlas_Bricks"
        );
    }

    #[test]
    fn atlas_suffixed_path_kept_as_is() {
        let def = make_def(
            "Granite",
            "Things/Building/Linked/Rock_Atlas",
            Some("Graphic_Single"),
        );
        assert_eq!(linked_atlas_path(&def), "Things/Building/Linked/Rock_Atlas");
    }

    #[test]
    fn effective_link_type_falls_back_to_basic() {
        let mut def = make_def("X", "p", None);
        def.graphic_data.link_type = LinkDrawerType::Transmitter;
        assert_eq!(effective_link_type(&def), LinkDrawerType::Basic);

        def.graphic_data.link_type = LinkDrawerType::CornerFiller;
        assert_eq!(effective_link_type(&def), LinkDrawerType::CornerFiller);

        def.graphic_data.link_type = LinkDrawerType::None;
        assert_eq!(effective_link_type(&def), LinkDrawerType::Basic);
    }
}
