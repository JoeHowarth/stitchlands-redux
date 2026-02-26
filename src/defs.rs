use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use glam::{Vec2, Vec3};
use roxmltree::{Document, Node};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl RgbaColor {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub struct GraphicData {
    pub tex_path: String,
    pub graphic_class: Option<String>,
    pub shader_type: Option<String>,
    pub color: RgbaColor,
    pub color_two: Option<RgbaColor>,
    pub draw_size: Vec2,
    pub draw_offset: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThingDef {
    pub def_name: String,
    pub graphic_data: GraphicData,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainDef {
    pub def_name: String,
    pub texture_path: String,
    pub edge_texture_path: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ApparelLayerDef {
    OnSkin,
    Middle,
    Shell,
    Belt,
    Overhead,
    EyeCover,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ApparelSkipFlagDef {
    None,
    Hair,
    Beard,
    Eyes,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ApparelWornDirectionDef {
    pub offset: Vec2,
    pub scale: Vec2,
}

impl Default for ApparelWornDirectionDef {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            scale: Vec2::ONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ApparelWornGraphicDef {
    pub render_utility_as_pack: bool,
    pub north: ApparelWornDirectionDef,
    pub east: ApparelWornDirectionDef,
    pub south: ApparelWornDirectionDef,
    pub west: ApparelWornDirectionDef,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ApparelDrawDataDef {
    pub north_layer: Option<f32>,
    pub east_layer: Option<f32>,
    pub south_layer: Option<f32>,
    pub west_layer: Option<f32>,
}

impl ApparelLayerDef {
    pub fn draw_order(self) -> i32 {
        match self {
            Self::OnSkin => 10,
            Self::Middle => 20,
            Self::Shell => 30,
            Self::Belt => 40,
            Self::Overhead => 50,
            Self::EyeCover => 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApparelDef {
    pub def_name: String,
    pub tex_path: String,
    pub layer: ApparelLayerDef,
    pub draw_size: Vec2,
    pub color: RgbaColor,
    pub covers_upper_head: bool,
    pub covers_full_head: bool,
    pub render_skip_flags: Option<Vec<ApparelSkipFlagDef>>,
    pub draw_data: ApparelDrawDataDef,
    pub worn_graphic: ApparelWornGraphicDef,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyTypeDefRender {
    pub def_name: String,
    pub head_offset: Vec2,
    pub body_graphic_scale: Vec2,
    pub body_naked_graphic_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeadTypeDefRender {
    pub def_name: String,
    pub graphic_path: String,
    pub narrow: bool,
    pub hair_mesh_size: Vec2,
    pub beard_mesh_size: Vec2,
    pub beard_offset: Vec3,
    pub beard_offset_x_east: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BeardDefRender {
    pub def_name: String,
    pub tex_path: String,
    pub no_graphic: bool,
    pub offset_narrow_east: Vec3,
    pub offset_narrow_south: Vec3,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HairDefRender {
    pub def_name: String,
    pub tex_path: String,
}

pub fn load_thing_defs(core_data_dir: &Path) -> Result<HashMap<String, ThingDef>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }

    let mut defs = HashMap::new();

    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let Some(ext) = entry.path().extension() else {
            continue;
        };
        if ext != "xml" {
            continue;
        }

        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;

        if let Ok(doc) = Document::parse(&xml) {
            parse_doc_thing_defs(&doc, &mut defs);
        }
    }

    Ok(defs)
}

pub fn load_terrain_defs(core_data_dir: &Path) -> Result<HashMap<String, TerrainDef>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }

    let mut defs = HashMap::new();
    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let Some(ext) = entry.path().extension() else {
            continue;
        };
        if ext != "xml" {
            continue;
        }

        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;
        if let Ok(doc) = Document::parse(&xml) {
            parse_doc_terrain_defs(&doc, &mut defs);
        }
    }

    Ok(defs)
}

pub fn load_apparel_defs(core_data_dir: &Path) -> Result<HashMap<String, ApparelDef>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }

    let mut defs = HashMap::new();
    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let Some(ext) = entry.path().extension() else {
            continue;
        };
        if ext != "xml" {
            continue;
        }

        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;
        if let Ok(doc) = Document::parse(&xml) {
            parse_doc_apparel_defs(&doc, &mut defs);
        }
    }

    Ok(defs)
}

pub fn load_body_type_defs(core_data_dir: &Path) -> Result<HashMap<String, BodyTypeDefRender>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }

    let mut defs = HashMap::new();
    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;
        if let Ok(doc) = Document::parse(&xml) {
            parse_doc_body_type_defs(&doc, &mut defs);
        }
    }
    Ok(defs)
}

pub fn load_head_type_defs(core_data_dir: &Path) -> Result<HashMap<String, HeadTypeDefRender>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }

    #[derive(Clone, Default)]
    struct RawHeadType {
        key: String,
        parent_name: Option<String>,
        def_name: Option<String>,
        graphic_path: Option<String>,
        narrow: Option<bool>,
        hair_mesh_size: Option<Vec2>,
        beard_mesh_size: Option<Vec2>,
        beard_offset: Option<Vec3>,
        beard_offset_x_east: Option<f32>,
    }

    let mut raw = HashMap::<String, RawHeadType>::new();
    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;
        let Ok(doc) = Document::parse(&xml) else {
            continue;
        };
        for node in doc.descendants().filter(|n| n.has_tag_name("HeadTypeDef")) {
            let mut record = RawHeadType::default();
            let name_attr = node.attribute("Name").map(str::to_string);
            record.parent_name = node.attribute("ParentName").map(str::to_string);
            record.def_name = child_text(node, "defName").map(str::to_string);
            record.graphic_path = child_text(node, "graphicPath").map(str::to_string);
            record.narrow = child_text(node, "narrow").map(|v| parse_bool(v).unwrap_or(false));
            record.hair_mesh_size = child_text(node, "hairMeshSize").and_then(parse_vec2_inline);
            record.beard_mesh_size = child_text(node, "beardMeshSize").and_then(parse_vec2_inline);
            record.beard_offset = child_text(node, "beardOffset").and_then(parse_vec3_inline);
            record.beard_offset_x_east =
                child_text(node, "beardOffsetXEast").and_then(|v| v.parse::<f32>().ok());
            let Some(key) = record.def_name.clone().or(name_attr) else {
                continue;
            };
            record.key = key.clone();
            raw.insert(key, record);
        }
    }

    fn resolve(
        key: &str,
        all: &HashMap<String, RawHeadType>,
        cache: &mut HashMap<String, HeadTypeDefRender>,
        stack: &mut Vec<String>,
    ) -> Option<HeadTypeDefRender> {
        if let Some(existing) = cache.get(key) {
            return Some(existing.clone());
        }
        if stack.iter().any(|k| k == key) {
            return None;
        }
        let raw = all.get(key)?;
        stack.push(key.to_string());
        let parent = raw
            .parent_name
            .as_ref()
            .and_then(|p| resolve(p, all, cache, stack));
        let resolved = HeadTypeDefRender {
            def_name: raw.def_name.clone().unwrap_or_else(|| key.to_string()),
            graphic_path: raw
                .graphic_path
                .clone()
                .or_else(|| parent.as_ref().map(|p| p.graphic_path.clone()))
                .unwrap_or_default(),
            narrow: raw
                .narrow
                .or_else(|| parent.as_ref().map(|p| p.narrow))
                .unwrap_or(false),
            hair_mesh_size: raw
                .hair_mesh_size
                .or_else(|| parent.as_ref().map(|p| p.hair_mesh_size))
                .unwrap_or(Vec2::new(1.5, 1.5)),
            beard_mesh_size: raw
                .beard_mesh_size
                .or_else(|| parent.as_ref().map(|p| p.beard_mesh_size))
                .unwrap_or(Vec2::new(1.5, 1.5)),
            beard_offset: raw
                .beard_offset
                .or_else(|| parent.as_ref().map(|p| p.beard_offset))
                .unwrap_or(Vec3::ZERO),
            beard_offset_x_east: raw
                .beard_offset_x_east
                .or_else(|| parent.as_ref().map(|p| p.beard_offset_x_east))
                .unwrap_or(0.0),
        };
        stack.pop();
        cache.insert(key.to_string(), resolved.clone());
        Some(resolved)
    }

    let mut out = HashMap::new();
    let mut resolved_cache = HashMap::new();
    for key in raw.keys() {
        let mut stack = Vec::new();
        if let Some(head) = resolve(key, &raw, &mut resolved_cache, &mut stack)
            && !head.def_name.is_empty()
            && !head.graphic_path.is_empty()
        {
            out.insert(head.def_name.clone(), head);
        }
    }
    Ok(out)
}

pub fn load_beard_defs(core_data_dir: &Path) -> Result<HashMap<String, BeardDefRender>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }
    let mut defs = HashMap::new();
    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;
        let Ok(doc) = Document::parse(&xml) else {
            continue;
        };
        for node in doc.descendants().filter(|n| n.has_tag_name("BeardDef")) {
            let Some(def_name) = child_text(node, "defName").map(str::to_string) else {
                continue;
            };
            let no_graphic = child_text(node, "noGraphic")
                .and_then(parse_bool)
                .unwrap_or(false);
            let tex_path = child_text(node, "texPath")
                .map(str::to_string)
                .unwrap_or_default();
            let offset_narrow_east = child_text(node, "offsetNarrowEast")
                .and_then(parse_vec3_inline)
                .unwrap_or(Vec3::ZERO);
            let offset_narrow_south = child_text(node, "offsetNarrowSouth")
                .and_then(parse_vec3_inline)
                .unwrap_or(Vec3::ZERO);
            defs.insert(
                def_name.clone(),
                BeardDefRender {
                    def_name,
                    tex_path,
                    no_graphic,
                    offset_narrow_east,
                    offset_narrow_south,
                },
            );
        }
    }
    Ok(defs)
}

pub fn load_hair_defs(core_data_dir: &Path) -> Result<HashMap<String, HairDefRender>> {
    let defs_dir = core_data_dir.join("Core").join("Defs");
    if !defs_dir.exists() {
        anyhow::bail!("Core defs dir not found: {}", defs_dir.display());
    }
    let mut defs = HashMap::new();
    for entry in WalkDir::new(&defs_dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        let xml = fs::read_to_string(entry.path())
            .with_context(|| format!("failed reading {}", entry.path().display()))?;
        let Ok(doc) = Document::parse(&xml) else {
            continue;
        };
        for node in doc.descendants().filter(|n| n.has_tag_name("HairDef")) {
            let Some(def_name) = child_text(node, "defName").map(str::to_string) else {
                continue;
            };
            let Some(tex_path) = child_text(node, "texPath").map(str::to_string) else {
                continue;
            };
            defs.insert(def_name.clone(), HairDefRender { def_name, tex_path });
        }
    }
    Ok(defs)
}

fn parse_doc_body_type_defs(doc: &Document<'_>, defs: &mut HashMap<String, BodyTypeDefRender>) {
    for node in doc.descendants().filter(|n| n.has_tag_name("BodyTypeDef")) {
        let Some(def_name) = child_text(node, "defName").map(str::to_string) else {
            continue;
        };
        let Some(head_offset) = child_text(node, "headOffset").and_then(parse_vec2_inline) else {
            continue;
        };
        let body_graphic_scale = child_text(node, "bodyGraphicScale")
            .and_then(parse_vec2_inline)
            .unwrap_or(Vec2::ONE);
        let Some(body_naked_graphic_path) =
            child_text(node, "bodyNakedGraphicPath").map(str::to_string)
        else {
            continue;
        };
        defs.insert(
            def_name.clone(),
            BodyTypeDefRender {
                def_name,
                head_offset,
                body_graphic_scale,
                body_naked_graphic_path,
            },
        );
    }
}

fn parse_doc_thing_defs(doc: &Document<'_>, defs: &mut HashMap<String, ThingDef>) {
    for node in doc.descendants().filter(|n| n.has_tag_name("ThingDef")) {
        if let Some(thing_def) = parse_thing_def(node) {
            defs.insert(thing_def.def_name.clone(), thing_def);
        }
    }
}

fn parse_doc_terrain_defs(doc: &Document<'_>, defs: &mut HashMap<String, TerrainDef>) {
    for node in doc.descendants().filter(|n| n.has_tag_name("TerrainDef")) {
        if let Some(terrain_def) = parse_terrain_def(node) {
            defs.insert(terrain_def.def_name.clone(), terrain_def);
        }
    }
}

fn parse_doc_apparel_defs(doc: &Document<'_>, defs: &mut HashMap<String, ApparelDef>) {
    for node in doc.descendants().filter(|n| n.has_tag_name("ThingDef")) {
        if let Some(apparel_def) = parse_apparel_def(node) {
            defs.insert(apparel_def.def_name.clone(), apparel_def);
        }
    }
}

fn parse_thing_def(node: Node<'_, '_>) -> Option<ThingDef> {
    let def_name = child_text(node, "defName")?.to_string();
    let graphic_node = node.children().find(|n| n.has_tag_name("graphicData"))?;
    let graphic_data = parse_graphic_data(graphic_node)?;

    Some(ThingDef {
        def_name,
        graphic_data,
    })
}

fn parse_terrain_def(node: Node<'_, '_>) -> Option<TerrainDef> {
    let def_name = child_text(node, "defName")?.to_string();
    let texture_path = child_text(node, "texturePath")
        .or_else(|| child_text(node, "texPath"))
        .map(str::to_string)?;
    let edge_texture_path = child_text(node, "edgePath")
        .or_else(|| child_text(node, "edgeTexturePath"))
        .map(str::to_string);

    Some(TerrainDef {
        def_name,
        texture_path,
        edge_texture_path,
    })
}

fn parse_apparel_def(node: Node<'_, '_>) -> Option<ApparelDef> {
    let def_name = child_text(node, "defName")?.to_string();
    let apparel_node = child_node(node, "apparel")?;
    let graphic_node = child_node(node, "graphicData")?;
    let graphic_data = parse_graphic_data(graphic_node)?;
    let tex_path = child_text(apparel_node, "wornGraphicPath").map(str::to_string)?;
    if !tex_path.starts_with("Things/Pawn/Humanlike/Apparel/") {
        return None;
    }

    let mut layer = ApparelLayerDef::OnSkin;
    if let Some(layers_node) = child_node(apparel_node, "layers") {
        for layer_name in list_text_values(layers_node) {
            if let Some(parsed_layer) = parse_apparel_layer_def(layer_name)
                && parsed_layer.draw_order() >= layer.draw_order()
            {
                layer = parsed_layer;
            }
        }
    }

    let mut covers_upper_head = false;
    let mut covers_full_head = false;
    if let Some(groups_node) = child_node(apparel_node, "bodyPartGroups") {
        for group in list_text_values(groups_node) {
            let lower = group.to_ascii_lowercase();
            if lower.contains("fullhead") {
                covers_full_head = true;
                covers_upper_head = true;
            } else if lower.contains("upperhead") {
                covers_upper_head = true;
            }
        }
    }

    let render_skip_flags = child_node(apparel_node, "renderSkipFlags").map(|skip_node| {
        list_text_values(skip_node)
            .filter_map(parse_apparel_skip_flag_def)
            .collect::<Vec<_>>()
    });
    let draw_data = child_node(apparel_node, "drawData")
        .map(parse_apparel_draw_data)
        .unwrap_or_default();
    let worn_graphic = child_node(apparel_node, "wornGraphicData")
        .map(parse_apparel_worn_graphic)
        .unwrap_or_default();

    Some(ApparelDef {
        def_name,
        tex_path,
        layer,
        draw_size: graphic_data.draw_size,
        color: graphic_data.color,
        covers_upper_head,
        covers_full_head,
        render_skip_flags,
        draw_data,
        worn_graphic,
    })
}

fn parse_graphic_data(node: Node<'_, '_>) -> Option<GraphicData> {
    let tex_path = child_text(node, "texPath")?.to_string();
    let graphic_class = child_text(node, "graphicClass").map(str::to_string);
    let shader_type = child_text(node, "shaderType").map(str::to_string);
    let color = child_text(node, "color")
        .and_then(parse_color)
        .unwrap_or(RgbaColor::WHITE);
    let color_two = child_text(node, "colorTwo").and_then(parse_color);
    let draw_size = child_node(node, "drawSize")
        .map(parse_vec2)
        .unwrap_or(Vec2::new(1.0, 1.0));
    let draw_offset = child_node(node, "drawOffset")
        .map(parse_vec3)
        .unwrap_or(Vec3::ZERO);

    Some(GraphicData {
        tex_path,
        graphic_class,
        shader_type,
        color,
        color_two,
        draw_size,
        draw_offset,
    })
}

fn child_text<'a>(node: Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    child_node(node, tag).and_then(|n| n.text()).map(str::trim)
}

fn child_node<'a>(node: Node<'a, 'a>, tag: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name(tag))
}

fn list_text_values<'a>(node: Node<'a, 'a>) -> impl Iterator<Item = &'a str> {
    node.children()
        .filter_map(|child| child.text())
        .map(str::trim)
}

fn parse_apparel_layer_def(input: &str) -> Option<ApparelLayerDef> {
    let name = input.rsplit('.').next().unwrap_or(input);
    match name {
        "OnSkin" => Some(ApparelLayerDef::OnSkin),
        "Middle" => Some(ApparelLayerDef::Middle),
        "Shell" => Some(ApparelLayerDef::Shell),
        "Belt" => Some(ApparelLayerDef::Belt),
        "Overhead" => Some(ApparelLayerDef::Overhead),
        "EyeCover" => Some(ApparelLayerDef::EyeCover),
        _ => None,
    }
}

fn parse_apparel_skip_flag_def(input: &str) -> Option<ApparelSkipFlagDef> {
    let name = input.rsplit('.').next().unwrap_or(input);
    match name {
        "None" => Some(ApparelSkipFlagDef::None),
        "Hair" => Some(ApparelSkipFlagDef::Hair),
        "Beard" => Some(ApparelSkipFlagDef::Beard),
        "Eyes" => Some(ApparelSkipFlagDef::Eyes),
        _ => None,
    }
}

fn parse_apparel_draw_data(node: Node<'_, '_>) -> ApparelDrawDataDef {
    let mut out = ApparelDrawDataDef::default();
    for child in node.children().filter(|c| c.is_element()) {
        let key = child.tag_name().name().to_ascii_lowercase();
        let layer = child_text(child, "layer").and_then(|t| t.parse::<f32>().ok());
        match key.as_str() {
            "datanorth" => out.north_layer = layer,
            "dataeast" => out.east_layer = layer,
            "datasouth" => out.south_layer = layer,
            "datawest" => out.west_layer = layer,
            _ => {}
        }
    }
    out
}

fn parse_apparel_worn_graphic(node: Node<'_, '_>) -> ApparelWornGraphicDef {
    let mut out = ApparelWornGraphicDef {
        render_utility_as_pack: child_text(node, "renderUtilityAsPack")
            .and_then(parse_bool)
            .unwrap_or(false),
        ..Default::default()
    };
    if let Some(north) = child_node(node, "north") {
        out.north = parse_apparel_worn_direction(north);
    }
    if let Some(east) = child_node(node, "east") {
        out.east = parse_apparel_worn_direction(east);
    }
    if let Some(south) = child_node(node, "south") {
        out.south = parse_apparel_worn_direction(south);
    }
    if let Some(west) = child_node(node, "west") {
        out.west = parse_apparel_worn_direction(west);
    }
    out
}

fn parse_apparel_worn_direction(node: Node<'_, '_>) -> ApparelWornDirectionDef {
    let offset = child_text(node, "offset")
        .and_then(parse_vec2_inline)
        .unwrap_or(Vec2::ZERO);
    let scale = child_text(node, "scale")
        .and_then(parse_vec2_inline)
        .unwrap_or(Vec2::ONE);
    ApparelWornDirectionDef { offset, scale }
}

fn parse_color(input: &str) -> Option<RgbaColor> {
    let cleaned = input.replace(',', " ");
    let mut parts = cleaned.split_whitespace();
    let r = parts.next()?.parse::<f32>().ok()?;
    let g = parts.next()?.parse::<f32>().ok()?;
    let b = parts.next()?.parse::<f32>().ok()?;
    let a = parts
        .next()
        .and_then(|p| p.parse::<f32>().ok())
        .unwrap_or(1.0);
    Some(RgbaColor { r, g, b, a })
}

fn parse_vec2(node: Node<'_, '_>) -> Vec2 {
    let x = child_text(node, "x")
        .and_then(|t| t.parse::<f32>().ok())
        .unwrap_or(1.0);
    let y = child_text(node, "y")
        .and_then(|t| t.parse::<f32>().ok())
        .unwrap_or(1.0);
    Vec2::new(x, y)
}

fn parse_vec3(node: Node<'_, '_>) -> Vec3 {
    let x = child_text(node, "x")
        .and_then(|t| t.parse::<f32>().ok())
        .unwrap_or(0.0);
    let y = child_text(node, "y")
        .and_then(|t| t.parse::<f32>().ok())
        .unwrap_or(0.0);
    let z = child_text(node, "z")
        .and_then(|t| t.parse::<f32>().ok())
        .unwrap_or(0.0);
    Vec3::new(x, y, z)
}

fn parse_vec2_inline(input: &str) -> Option<Vec2> {
    let cleaned = input.trim().trim_start_matches('(').trim_end_matches(')');
    let mut parts = cleaned.split(',').map(|p| p.trim().parse::<f32>().ok());
    let x = parts.next().flatten()?;
    let y = parts.next().flatten()?;
    Some(Vec2::new(x, y))
}

fn parse_vec3_inline(input: &str) -> Option<Vec3> {
    let cleaned = input.trim().trim_start_matches('(').trim_end_matches(')');
    let mut parts = cleaned.split(',').map(|p| p.trim().parse::<f32>().ok());
    let x = parts.next().flatten()?;
    let y = parts.next().flatten()?;
    let z = parts.next().flatten()?;
    Some(Vec3::new(x, y, z))
}

fn parse_bool(input: &str) -> Option<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_color_formats() {
        let c1 = parse_color("1 0.5 0").unwrap();
        assert_eq!(
            c1,
            RgbaColor {
                r: 1.0,
                g: 0.5,
                b: 0.0,
                a: 1.0
            }
        );

        let c2 = parse_color("0.1, 0.2, 0.3, 0.4").unwrap();
        assert_eq!(
            c2,
            RgbaColor {
                r: 0.1,
                g: 0.2,
                b: 0.3,
                a: 0.4,
            }
        );
    }

    #[test]
    fn parses_minimal_thingdef() {
        let xml = r#"
        <Defs>
            <ThingDef>
                <defName>TestThing</defName>
                <graphicData>
                    <texPath>Things/Test</texPath>
                    <drawSize><x>2.0</x><y>3.0</y></drawSize>
                    <drawOffset><x>0.1</x><y>0.2</y><z>0.3</z></drawOffset>
                </graphicData>
            </ThingDef>
        </Defs>
        "#;
        let doc = Document::parse(xml).unwrap();
        let mut defs = HashMap::new();
        parse_doc_thing_defs(&doc, &mut defs);

        let thing = defs.get("TestThing").unwrap();
        assert_eq!(thing.graphic_data.tex_path, "Things/Test");
        assert_eq!(thing.graphic_data.draw_size, Vec2::new(2.0, 3.0));
        assert_eq!(thing.graphic_data.draw_offset, Vec3::new(0.1, 0.2, 0.3));
    }

    #[test]
    fn parses_minimal_terraindef() {
        let xml = r#"
        <Defs>
            <TerrainDef>
                <defName>SoilRich</defName>
                <texturePath>Terrain/Surfaces/SoilRich</texturePath>
                <edgePath>Terrain/Edges/Soil</edgePath>
            </TerrainDef>
        </Defs>
        "#;
        let doc = Document::parse(xml).unwrap();
        let mut defs = HashMap::new();
        parse_doc_terrain_defs(&doc, &mut defs);

        let terrain = defs.get("SoilRich").unwrap();
        assert_eq!(terrain.texture_path, "Terrain/Surfaces/SoilRich");
        assert_eq!(
            terrain.edge_texture_path.as_deref(),
            Some("Terrain/Edges/Soil")
        );
    }

    #[test]
    fn parses_apparel_layer_and_head_coverage() {
        let xml = r#"
        <Defs>
            <ThingDef>
                <defName>Apparel_TestHelmet</defName>
                <graphicData>
                    <texPath>Things/Pawn/Humanlike/Apparel/TestHelmet/TestHelmet</texPath>
                    <drawSize><x>1.1</x><y>1.1</y></drawSize>
                    <color>0.8 0.8 0.9 1.0</color>
                </graphicData>
                <apparel>
                    <wornGraphicPath>Things/Pawn/Humanlike/Apparel/TestHelmet/TestHelmet</wornGraphicPath>
                    <layers>
                        <li>OnSkin</li>
                        <li>Overhead</li>
                    </layers>
                    <bodyPartGroups>
                        <li>UpperHead</li>
                        <li>FullHead</li>
                    </bodyPartGroups>
                </apparel>
            </ThingDef>
        </Defs>
        "#;
        let doc = Document::parse(xml).unwrap();
        let mut defs = HashMap::new();
        parse_doc_apparel_defs(&doc, &mut defs);
        let apparel = defs.get("Apparel_TestHelmet").unwrap();
        assert_eq!(apparel.layer, ApparelLayerDef::Overhead);
        assert!(apparel.covers_upper_head);
        assert!(apparel.covers_full_head);
        assert_eq!(
            apparel.tex_path,
            "Things/Pawn/Humanlike/Apparel/TestHelmet/TestHelmet"
        );
    }

    #[test]
    fn resolves_head_type_inheritance() {
        let xml = r#"
        <Defs>
            <HeadTypeDef Name="NarrowBase" Abstract="True">
                <narrow>true</narrow>
                <beardOffset>(0, 0, -0.05)</beardOffset>
                <beardOffsetXEast>-0.05</beardOffsetXEast>
            </HeadTypeDef>
            <HeadTypeDef ParentName="NarrowBase">
                <defName>Male_NarrowNormal</defName>
                <graphicPath>Things/Pawn/Humanlike/Heads/Male/Male_Narrow_Normal</graphicPath>
            </HeadTypeDef>
        </Defs>
        "#;
        let doc = Document::parse(xml).unwrap();
        let mut raw = HashMap::<String, (Option<String>, Option<String>, Option<String>)>::new();
        for node in doc.descendants().filter(|n| n.has_tag_name("HeadTypeDef")) {
            let key = node
                .attribute("Name")
                .map(str::to_string)
                .or_else(|| child_text(node, "defName").map(str::to_string))
                .unwrap();
            raw.insert(
                key,
                (
                    node.attribute("ParentName").map(str::to_string),
                    child_text(node, "defName").map(str::to_string),
                    child_text(node, "graphicPath").map(str::to_string),
                ),
            );
        }
        assert!(raw.contains_key("NarrowBase"));
        assert!(raw.contains_key("Male_NarrowNormal"));
    }
}
