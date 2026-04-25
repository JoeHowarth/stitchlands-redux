use std::path::Path;

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use log::warn;

use crate::assets::AssetResolver;
use crate::cell::Cell;
use crate::defs::{GraphicKind, ThingDef};
use crate::linking::{
    LinkDrawerType, LinkFlags, TerrainEdgeType, atlas_uv_rect, corner_filler_positions, link_index,
    perimeter_alphas_from_neighbor_matches,
};
use crate::renderer::{EdgeFan, EdgeSpriteInput, EdgeType, EdgeVertex, SpriteParams};
use crate::viewer::RenderSprite;
use crate::world::{
    DEPTH_TERRAIN_EDGE, DEPTH_WALL, DEPTH_WALL_CORNER, ThingState, WorldState, cardinal_neighbors,
    diagonal_neighbors, neighbors_8,
};

use super::common::DefSet;

/// Step size for per-cell noise-seed offsets. Small irrational increments stop
/// the RoughAlphaAdd sampler from tiling visibly across cells.
const EDGE_NOISE_STEP: f32 = 0.31;

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
    _data_dir: &Path,
    asset_resolver: &mut AssetResolver,
    defs: &DefSet<'_>,
    thing: &ThingState,
    thing_def: &ThingDef,
    world: &WorldState,
    strict_missing: bool,
) -> Result<Vec<RenderSprite>> {
    let atlas_path = linked_atlas_path(thing_def);
    let resolved = asset_resolver
        .resolve_texture_path(&atlas_path)
        .with_context(|| {
            format!(
                "resolving linked atlas '{}' for '{}'",
                atlas_path, thing_def.def_name
            )
        })?;
    if strict_missing && resolved.used_fallback() {
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
        image: resolved.image.clone(),
        params: SpriteParams {
            world_pos: Vec3::new(cell.x as f32 + 0.5, cell.z as f32 + 0.5, DEPTH_WALL),
            size: Vec2::new(1.0, 1.0),
            tint,
            uv_rect,
        },
        used_fallback: resolved.used_fallback(),
        pawn_id: None,
        is_water: false,
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
                image: resolved.image.clone(),
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
                is_water: false,
                used_fallback: resolved.used_fallback(),
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
    if thing_def.graphic_data.kind == GraphicKind::Appearances {
        format!("{tex_path}/{basename}_Atlas_Bricks")
    } else {
        tex_path.to_string()
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

/// Pure emission logic: for each cell, collect one contribution per unique
/// neighboring terrain whose `render_precedence >= self.render_precedence`
/// (matching RimWorld's `Verse/SectionLayer_Terrain.cs:86`) and whose
/// `edge_type` is not `None` or `Hard`. Multiple neighbor slots sharing the
/// same def merge into a single contribution.
///
/// Extracted as a pure function so emission tests don't need an asset resolver.
pub(crate) struct TerrainEdgeContribution {
    pub cell: Cell,
    pub neighbor_def_name: String,
    pub neighbor_texture_path: String,
    pub perimeter_alphas: [f32; 8],
    pub edge_type: EdgeType,
}

pub(crate) fn compute_terrain_edge_contributions(
    defs: &DefSet<'_>,
    world: &WorldState,
) -> Result<Vec<TerrainEdgeContribution>> {
    struct Accum {
        neighbor_def_name: String,
        neighbor_texture_path: String,
        edge_type: EdgeType,
        matches: [bool; 8],
    }

    let mut out = Vec::new();
    for z in 0..world.height() {
        for x in 0..world.width() {
            let cell = Cell::new(x as i32, z as i32);
            let Some(self_tile) = world.terrain_at(cell) else {
                continue;
            };
            let self_def = defs
                .terrain_defs
                .get(&self_tile.terrain_def)
                .with_context(|| format!("missing TerrainDef '{}'", self_tile.terrain_def))?;

            let mut accum: Vec<Accum> = Vec::new();
            for (i, neighbor_cell) in neighbors_8(cell).iter().enumerate() {
                let Some(neighbor_tile) = world.terrain_at(*neighbor_cell) else {
                    continue;
                };
                let neighbor_def = defs
                    .terrain_defs
                    .get(&neighbor_tile.terrain_def)
                    .with_context(|| {
                        format!("missing TerrainDef '{}'", neighbor_tile.terrain_def)
                    })?;
                if neighbor_def.def_name == self_def.def_name {
                    continue;
                }
                // Two water terrains meeting: the base water pass already
                // paints both cells; an edge fan between them would overlay
                // the higher-precedence water's ramp on the lower one and
                // read as a muddy band inside the water body. Skip.
                if self_def.water_depth_shader.is_some()
                    && neighbor_def.water_depth_shader.is_some()
                {
                    continue;
                }
                let edge_type = match neighbor_def.edge_type {
                    TerrainEdgeType::None | TerrainEdgeType::Hard => continue,
                    TerrainEdgeType::FadeRough => EdgeType::FadeRough,
                    TerrainEdgeType::Water => EdgeType::Water,
                };
                if neighbor_def.render_precedence < self_def.render_precedence {
                    continue;
                }
                match accum
                    .iter_mut()
                    .find(|c| c.neighbor_def_name == neighbor_def.def_name)
                {
                    Some(existing) => {
                        existing.matches[i] = true;
                    }
                    None => {
                        let mut matches = [false; 8];
                        matches[i] = true;
                        accum.push(Accum {
                            neighbor_def_name: neighbor_def.def_name.clone(),
                            neighbor_texture_path: neighbor_def.texture_path.clone(),
                            edge_type,
                            matches,
                        });
                    }
                }
            }
            for a in accum {
                out.push(TerrainEdgeContribution {
                    cell,
                    neighbor_def_name: a.neighbor_def_name,
                    neighbor_texture_path: a.neighbor_texture_path,
                    perimeter_alphas: perimeter_alphas_from_neighbor_matches(a.matches),
                    edge_type: a.edge_type,
                });
            }
        }
    }
    Ok(out)
}

/// Cell-local positions of the 9 fan vertices. Index layout matches
/// `crate::linking::perimeter_alphas_from_neighbor_matches`:
/// 0 S mid, 1 SW, 2 W mid, 3 NW, 4 N mid, 5 NE, 6 E mid, 7 SE, 8 center.
const FAN_LOCAL_XY: [(f32, f32); 9] = [
    (0.5, 0.0),
    (0.0, 0.0),
    (0.0, 0.5),
    (0.0, 1.0),
    (0.5, 1.0),
    (1.0, 1.0),
    (1.0, 0.5),
    (1.0, 0.0),
    (0.5, 0.5),
];

/// For each cell, emit one fan per unique neighboring terrain whose
/// `render_precedence >= self.render_precedence`.
pub fn emit_terrain_edge_sprites(
    _data_dir: &Path,
    asset_resolver: &mut AssetResolver,
    defs: &DefSet<'_>,
    world: &WorldState,
    strict_missing: bool,
) -> Result<Vec<EdgeSpriteInput>> {
    let contributions = compute_terrain_edge_contributions(defs, world)?;
    let mut out = Vec::with_capacity(contributions.len());
    for contribution in contributions {
        let resolved = asset_resolver
            .resolve_texture_path(&contribution.neighbor_texture_path)
            .with_context(|| {
                format!(
                    "resolving terrain edge texture '{}' for '{}'",
                    contribution.neighbor_texture_path, contribution.neighbor_def_name
                )
            })?;
        if strict_missing && resolved.used_fallback() {
            anyhow::bail!(
                "missing terrain edge texture '{}' for '{}'",
                contribution.neighbor_texture_path,
                contribution.neighbor_def_name
            );
        }
        let cell_x = contribution.cell.x as f32;
        let cell_z = contribution.cell.z as f32;
        let noise_seed = [cell_x * EDGE_NOISE_STEP, cell_z * EDGE_NOISE_STEP];
        let edge_type = contribution.edge_type as u32;
        let tint = [1.0, 1.0, 1.0, 1.0];

        let mut vertices = [EdgeVertex::default(); 9];
        for (i, &(lx, ly)) in FAN_LOCAL_XY.iter().enumerate() {
            let alpha = if i < 8 {
                contribution.perimeter_alphas[i]
            } else {
                0.0
            };
            vertices[i] = EdgeVertex {
                world_pos: [cell_x + lx, cell_z + ly, DEPTH_TERRAIN_EDGE],
                uv: [lx, 1.0 - ly],
                alpha,
                noise_seed,
                tint,
                edge_type,
                _pad: 0,
            };
        }
        out.push(EdgeSpriteInput {
            image: resolved.image,
            fan: EdgeFan { vertices },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::defs::{
        ApparelDef, BeardDefRender, BodyTypeDefRender, GraphicData, HairDefRender,
        HeadTypeDefRender, RgbaColor, TerrainDef,
    };
    use crate::fixtures::{MapSpec, SceneFixture, TerrainCell};
    use crate::world::world_from_fixture;

    fn make_def(def_name: &str, tex_path: &str, kind: GraphicKind) -> ThingDef {
        ThingDef {
            def_name: def_name.to_string(),
            graphic_data: GraphicData {
                tex_path: tex_path.to_string(),
                kind,
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
            GraphicKind::Appearances,
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
            GraphicKind::Single,
        );
        assert_eq!(linked_atlas_path(&def), "Things/Building/Linked/Rock_Atlas");
    }

    #[test]
    fn effective_link_type_falls_back_to_basic() {
        let mut def = make_def("X", "p", GraphicKind::Single);
        def.graphic_data.link_type = LinkDrawerType::Transmitter;
        assert_eq!(effective_link_type(&def), LinkDrawerType::Basic);

        def.graphic_data.link_type = LinkDrawerType::CornerFiller;
        assert_eq!(effective_link_type(&def), LinkDrawerType::CornerFiller);

        def.graphic_data.link_type = LinkDrawerType::None;
        assert_eq!(effective_link_type(&def), LinkDrawerType::Basic);
    }

    fn make_terrain(
        def_name: &str,
        edge_type: TerrainEdgeType,
        render_precedence: i32,
    ) -> TerrainDef {
        TerrainDef {
            def_name: def_name.to_string(),
            texture_path: format!("Terrain/{def_name}"),
            edge_texture_path: None,
            edge_type,
            render_precedence,
            water_depth_shader: None,
            water_depth_shader_parameters: Vec::new(),
        }
    }

    fn make_water(def_name: &str, render_precedence: i32) -> TerrainDef {
        TerrainDef {
            def_name: def_name.to_string(),
            texture_path: format!("Terrain/{def_name}"),
            edge_texture_path: None,
            edge_type: TerrainEdgeType::Water,
            render_precedence,
            water_depth_shader: Some("Map/WaterDepth".to_string()),
            water_depth_shader_parameters: Vec::new(),
        }
    }

    fn build_world(width: usize, height: usize, tiles: &[&str]) -> WorldState {
        assert_eq!(tiles.len(), width * height);
        world_from_fixture(&SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width,
                height,
                terrain: tiles
                    .iter()
                    .map(|name| TerrainCell {
                        terrain_def: (*name).to_string(),
                    })
                    .collect(),
            },
            things: Vec::new(),
            pawns: Vec::new(),
            camera: None,
        })
    }

    fn def_set<'a>(
        thing_defs: &'a HashMap<String, ThingDef>,
        terrain_defs: &'a HashMap<String, TerrainDef>,
        apparel_defs: &'a HashMap<String, ApparelDef>,
        body_type_defs: &'a HashMap<String, BodyTypeDefRender>,
        head_type_defs: &'a HashMap<String, HeadTypeDefRender>,
        beard_defs: &'a HashMap<String, BeardDefRender>,
        hair_defs: &'a HashMap<String, HairDefRender>,
    ) -> DefSet<'a> {
        DefSet {
            thing_defs,
            terrain_defs,
            apparel_defs,
            body_type_defs,
            head_type_defs,
            beard_defs,
            hair_defs,
        }
    }

    fn contributions(
        terrain_defs: &HashMap<String, TerrainDef>,
        world: &WorldState,
    ) -> Vec<TerrainEdgeContribution> {
        let thing_defs = HashMap::new();
        let apparel_defs = HashMap::new();
        let body_type_defs = HashMap::new();
        let head_type_defs = HashMap::new();
        let beard_defs = HashMap::new();
        let hair_defs = HashMap::new();
        let defs = def_set(
            &thing_defs,
            terrain_defs,
            &apparel_defs,
            &body_type_defs,
            &head_type_defs,
            &beard_defs,
            &hair_defs,
        );
        compute_terrain_edge_contributions(&defs, world).expect("contributions")
    }

    fn opaque(indices: &[usize]) -> [f32; 8] {
        let mut a = [0.0; 8];
        for &i in indices {
            a[i] = 1.0;
        }
        a
    }

    #[test]
    fn higher_precedence_neighbor_emits_onto_lower() {
        // 3x3 Soil with a single Concrete in the center (70 > 340 is false, so
        // Concrete is LOWER than Soil; flip the other way).
        // Actually: Soil=340 FadeRough, Concrete=70 Hard. Soil fades onto
        // Concrete. Here we put Concrete in center (precedence 70), ring of
        // Soil — Soil should emit onto Concrete (which has lower precedence).
        let mut defs = HashMap::new();
        defs.insert(
            "Soil".to_string(),
            make_terrain("Soil", TerrainEdgeType::FadeRough, 340),
        );
        defs.insert(
            "Concrete".to_string(),
            make_terrain("Concrete", TerrainEdgeType::Hard, 70),
        );

        let world = build_world(
            3,
            3,
            &[
                "Soil", "Soil", "Soil", //
                "Soil", "Concrete", "Soil", //
                "Soil", "Soil", "Soil",
            ],
        );

        let contribs = contributions(&defs, &world);
        // Concrete center sees all 8 Soil neighbors; self-def Soil cells in
        // the ring don't emit (same def as their Soil neighbors, Concrete is
        // Hard so it's skipped). One contribution, fully-lit perimeter.
        assert_eq!(contribs.len(), 1);
        let c = &contribs[0];
        assert_eq!(c.cell, Cell::new(1, 1));
        assert_eq!(c.neighbor_def_name, "Soil");
        assert_eq!(c.perimeter_alphas, [1.0; 8]);
        assert_eq!(c.edge_type, EdgeType::FadeRough);
    }

    #[test]
    fn equal_precedence_cross_emits() {
        let mut defs = HashMap::new();
        defs.insert(
            "Soil".to_string(),
            make_terrain("Soil", TerrainEdgeType::FadeRough, 340),
        );
        // Two distinct defs sharing the same precedence — RimWorld's `>=`
        // rule means they cross-emit onto each other.
        defs.insert(
            "Sand".to_string(),
            make_terrain("Sand", TerrainEdgeType::FadeRough, 340),
        );

        let world = build_world(2, 1, &["Soil", "Sand"]);
        let contribs = contributions(&defs, &world);
        assert_eq!(contribs.len(), 2);
        // Sort by cell for determinism.
        let mut by_cell: Vec<_> = contribs.iter().collect();
        by_cell.sort_by_key(|c| (c.cell.x, c.cell.z));
        // Soil at (0,0) has E neighbor Sand: k=6 cardinal lights 5,6,7.
        assert_eq!(by_cell[0].cell, Cell::new(0, 0));
        assert_eq!(by_cell[0].neighbor_def_name, "Sand");
        assert_eq!(by_cell[0].perimeter_alphas, opaque(&[5, 6, 7]));
        // Sand at (1,0) has W neighbor Soil: k=2 cardinal lights 1,2,3.
        assert_eq!(by_cell[1].cell, Cell::new(1, 0));
        assert_eq!(by_cell[1].neighbor_def_name, "Soil");
        assert_eq!(by_cell[1].perimeter_alphas, opaque(&[1, 2, 3]));
    }

    #[test]
    fn neighbor_with_edge_type_none_skipped() {
        let mut defs = HashMap::new();
        defs.insert(
            "Underwall".to_string(),
            make_terrain("Underwall", TerrainEdgeType::None, 0),
        );
        defs.insert(
            "Soil".to_string(),
            make_terrain("Soil", TerrainEdgeType::FadeRough, 340),
        );
        let world = build_world(2, 1, &["Underwall", "Soil"]);
        // Soil (340) > Underwall (0), but Soil touching Underwall emits onto
        // Underwall (precedence 0). Soil has edge_type FadeRough, so the
        // Underwall cell (0,0) sees a higher-precedence Soil neighbor E ->
        // emit. Symmetric direction (Soil sees Underwall with edge_type None
        // on W): nothing emitted.
        let contribs = contributions(&defs, &world);
        assert_eq!(contribs.len(), 1);
        assert_eq!(contribs[0].cell, Cell::new(0, 0));
        assert_eq!(contribs[0].neighbor_def_name, "Soil");
        // E neighbor (k=6 cardinal) lights verts 5, 6, 7.
        assert_eq!(contribs[0].perimeter_alphas, opaque(&[5, 6, 7]));
    }

    #[test]
    fn distinct_neighbor_defs_produce_separate_contributions() {
        let mut defs = HashMap::new();
        defs.insert(
            "Concrete".to_string(),
            make_terrain("Concrete", TerrainEdgeType::Hard, 70),
        );
        defs.insert(
            "Soil".to_string(),
            make_terrain("Soil", TerrainEdgeType::FadeRough, 340),
        );
        defs.insert(
            "Water".to_string(),
            make_terrain("Water", TerrainEdgeType::Water, 394),
        );
        // 3x1: Soil | Concrete | Water. Concrete is center; Soil (340) > 70
        // and Water (394) > 70, both emit separate contributions onto it.
        let world = build_world(3, 1, &["Soil", "Concrete", "Water"]);
        let contribs = contributions(&defs, &world);
        assert_eq!(contribs.len(), 2);
        let mut seen: Vec<_> = contribs
            .iter()
            .map(|c| {
                (
                    c.cell,
                    c.neighbor_def_name.as_str(),
                    c.perimeter_alphas,
                    c.edge_type,
                )
            })
            .collect();
        seen.sort_by(|a, b| a.1.cmp(b.1));
        // Soil neighbor on W of Concrete (1,0): k=2 cardinal lights 1,2,3.
        assert_eq!(seen[0].0, Cell::new(1, 0));
        assert_eq!(seen[0].1, "Soil");
        assert_eq!(seen[0].2, opaque(&[1, 2, 3]));
        assert_eq!(seen[0].3, EdgeType::FadeRough);
        // Water neighbor on E of Concrete (1,0): k=6 cardinal lights 5,6,7.
        assert_eq!(seen[1].0, Cell::new(1, 0));
        assert_eq!(seen[1].1, "Water");
        assert_eq!(seen[1].2, opaque(&[5, 6, 7]));
        assert_eq!(seen[1].3, EdgeType::Water);
    }

    #[test]
    fn water_depth_terrains_do_not_emit_edges_onto_each_other() {
        let mut defs = HashMap::new();
        defs.insert("WaterShallow".to_string(), make_water("WaterShallow", 394));
        defs.insert("WaterDeep".to_string(), make_water("WaterDeep", 395));

        let world = build_world(2, 1, &["WaterShallow", "WaterDeep"]);
        let contribs = contributions(&defs, &world);

        assert!(
            contribs.is_empty(),
            "water-depth terrains should be handled by the water surface pass, not edge fans"
        );
    }

    #[test]
    fn convex_corner_produces_corner_only_alpha() {
        // 3x3 base Soil (precedence 340) with a single Ice cell at the NE
        // corner (precedence 380). The base cell (1,1) sees Ice only on its
        // NE diagonal slot — no cardinal matches.
        let mut defs = HashMap::new();
        defs.insert(
            "Soil".to_string(),
            make_terrain("Soil", TerrainEdgeType::FadeRough, 340),
        );
        defs.insert(
            "Ice".to_string(),
            make_terrain("Ice", TerrainEdgeType::FadeRough, 380),
        );
        let world = build_world(
            3,
            3,
            &[
                "Soil", "Soil", "Soil", //
                "Soil", "Soil", "Soil", //
                "Soil", "Soil", "Ice",
            ],
        );
        let contribs = contributions(&defs, &world);
        let base = contribs
            .iter()
            .find(|c| c.cell == Cell::new(1, 1))
            .expect("base cell contribution");
        assert_eq!(base.neighbor_def_name, "Ice");
        // NE is k=5 diagonal — lights only vertex 5.
        assert_eq!(base.perimeter_alphas, opaque(&[5]));
    }
}
