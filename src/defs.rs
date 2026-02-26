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

    Some(ApparelDef {
        def_name,
        tex_path: graphic_data.tex_path,
        layer,
        draw_size: graphic_data.draw_size,
        color: graphic_data.color,
        covers_upper_head,
        covers_full_head,
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
                    <texPath>Things/Apparel/Headgear/TestHelmet</texPath>
                    <drawSize><x>1.1</x><y>1.1</y></drawSize>
                    <color>0.8 0.8 0.9 1.0</color>
                </graphicData>
                <apparel>
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
        assert_eq!(apparel.tex_path, "Things/Apparel/Headgear/TestHelmet");
    }
}
