use glam::{Vec2, Vec3};

use super::graph::{AnchorKind, GraphNode, NodePayload};
use super::model::{ApparelLayer, OverlayAnchor, PawnComposeConfig, PawnRenderInput};
use super::parms::{PawnDrawParms, RenderSkipFlag};
use super::rules::should_draw_hediff_overlay;
use super::tree::{PawnNode, PawnNodeKind};
use super::workers;

#[derive(Debug, Clone)]
pub struct PawnComposition {
    pub nodes: Vec<PawnNode>,
}

pub fn compose_pawn(input: &PawnRenderInput, config: &PawnComposeConfig) -> PawnComposition {
    let parms = PawnDrawParms::from_inputs(input.facing, input.draw_flags, &input.apparel);
    let graph = build_graph(input, &parms);
    let nodes = evaluate_graph(input, config, graph);
    PawnComposition { nodes }
}

fn build_graph(input: &PawnRenderInput, parms: &PawnDrawParms) -> Vec<GraphNode> {
    let mut out = Vec::new();
    let mut order = 0usize;

    out.push(GraphNode {
        id: format!("{}::Body", input.label),
        kind: PawnNodeKind::Body,
        anchor: AnchorKind::Body,
        order,
        payload: NodePayload::Body,
    });
    order += 1;

    if parms.draw_flags.head_stump {
        if input.stump_tex_path.is_some() {
            out.push(GraphNode {
                id: format!("{}::Stump", input.label),
                kind: PawnNodeKind::Stump,
                anchor: AnchorKind::Head,
                order,
                payload: NodePayload::Stump,
            });
            order += 1;
        }
    } else if !parms.draw_flags.hide_head {
        if input.head_tex_path.is_some() {
            out.push(GraphNode {
                id: format!("{}::Head", input.label),
                kind: PawnNodeKind::Head,
                anchor: AnchorKind::Head,
                order,
                payload: NodePayload::Head,
            });
            order += 1;
        }

        if !parms.skip(RenderSkipFlag::Hair) && input.hair_tex_path.is_some() {
            out.push(GraphNode {
                id: format!("{}::Hair", input.label),
                kind: PawnNodeKind::Hair,
                anchor: AnchorKind::Head,
                order,
                payload: NodePayload::Hair,
            });
            order += 1;
        }

        if !parms.skip(RenderSkipFlag::Beard) && input.beard_tex_path.is_some() {
            out.push(GraphNode {
                id: format!("{}::Beard", input.label),
                kind: PawnNodeKind::Beard,
                anchor: AnchorKind::Head,
                order,
                payload: NodePayload::Beard,
            });
            order += 1;
        }
    }

    let mut ordered_apparel: Vec<_> = input.apparel.iter().enumerate().collect();
    ordered_apparel.sort_by(|(a_idx, a), (b_idx, b)| {
        a.layer
            .draw_order()
            .cmp(&b.layer.draw_order())
            .then(a_idx.cmp(b_idx))
    });
    for (_source_index, apparel) in ordered_apparel.into_iter() {
        debug_assert!(ApparelLayer::ALL.contains(&apparel.layer));
        let anchor = if matches!(
            apparel.layer,
            ApparelLayer::Overhead | ApparelLayer::EyeCover
        ) {
            AnchorKind::Head
        } else {
            AnchorKind::Body
        };
        out.push(GraphNode {
            id: format!("{}::Apparel::{}", input.label, apparel.label),
            kind: PawnNodeKind::Apparel,
            anchor,
            order,
            payload: NodePayload::Apparel(apparel.clone()),
        });
        order += 1;
    }

    for overlay in &input.hediff_overlays {
        if !should_draw_hediff_overlay(overlay, parms.facing, &input.present_body_part_groups) {
            continue;
        }
        let anchor = if matches!(overlay.anchor, OverlayAnchor::Head) {
            AnchorKind::Head
        } else {
            AnchorKind::Body
        };
        out.push(GraphNode {
            id: format!("{}::Hediff::{}", input.label, overlay.label),
            kind: PawnNodeKind::Hediff,
            anchor,
            order,
            payload: NodePayload::Hediff(overlay.clone()),
        });
        order += 1;
    }

    out
}

fn evaluate_graph(
    input: &PawnRenderInput,
    config: &PawnComposeConfig,
    graph: Vec<GraphNode>,
) -> Vec<PawnNode> {
    let base = Vec2::new(
        input.world_pos.x + workers::facing_x_offset(input.facing),
        input.world_pos.y,
    );

    let mut apparel_index = 0usize;
    let mut hediff_index = 0usize;

    let mut out = Vec::with_capacity(graph.len());
    for g in graph {
        let anchor = workers::anchor_offset(g.anchor, input.facing, input.body_type);
        let (tex_path, size, tint, extra_offset, z) = match &g.payload {
            NodePayload::Body => (
                input.body_tex_path.clone(),
                input.body_size,
                input.tint,
                Vec2::ZERO,
                config.layering.body_z,
            ),
            NodePayload::Head => (
                input.head_tex_path.clone().unwrap_or_default(),
                input.head_size,
                input.tint,
                workers::head_extra_offset(input.facing, input.head_type, config.layering),
                config.layering.head_z,
            ),
            NodePayload::Stump => (
                input.stump_tex_path.clone().unwrap_or_default(),
                input.stump_size,
                input.tint,
                Vec2::new(0.0, config.layering.stump_y_offset),
                config.layering.head_z,
            ),
            NodePayload::Hair => (
                input.hair_tex_path.clone().unwrap_or_default(),
                input.hair_size,
                input.tint,
                Vec2::new(0.0, config.layering.hair_y_offset),
                config.layering.hair_z,
            ),
            NodePayload::Beard => (
                input.beard_tex_path.clone().unwrap_or_default(),
                input.beard_size,
                input.tint,
                workers::beard_extra_offset(
                    input.facing,
                    input.head_type,
                    input.beard_type,
                    config.layering,
                ),
                config.layering.beard_z,
            ),
            NodePayload::Apparel(apparel) => {
                let mut z = workers::apparel_z(config.layering, apparel.layer, apparel_index);
                if let Some(layer) = apparel.layer_override {
                    z = config.layering.body_z + workers::layer_to_z_delta(layer);
                } else if apparel.render_as_pack {
                    z = match input.facing {
                        super::model::PawnFacing::South => config.layering.body_z - 0.03,
                        super::model::PawnFacing::North => config.layering.body_z + 0.03,
                        super::model::PawnFacing::East | super::model::PawnFacing::West => {
                            config.layering.body_z + 0.01
                        }
                    };
                }
                apparel_index += 1;
                (
                    apparel.tex_path.clone(),
                    Vec2::new(
                        apparel.draw_size.x * apparel.draw_scale.x,
                        apparel.draw_size.y * apparel.draw_scale.y,
                    ),
                    apparel.tint,
                    workers::apparel_offset(apparel.layer, config.layering) + apparel.draw_offset,
                    z,
                )
            }
            NodePayload::Hediff(overlay) => {
                let anchored_to_head = matches!(overlay.anchor, OverlayAnchor::Head);
                let z = workers::hediff_z(
                    config.layering,
                    anchored_to_head,
                    overlay.layer_offset,
                    hediff_index,
                );
                hediff_index += 1;
                (
                    overlay.tex_path.clone(),
                    overlay.draw_size,
                    overlay.tint,
                    if anchored_to_head {
                        workers::hediff_offset_head(config.layering)
                    } else {
                        Vec2::ZERO
                    },
                    z,
                )
            }
        };

        let world = Vec3::new(
            base.x + anchor.x + extra_offset.x,
            base.y + anchor.y + extra_offset.y,
            z,
        );
        out.push(PawnNode {
            id: g.id,
            kind: g.kind,
            tex_path,
            world_pos: world,
            size,
            tint,
            z,
            order: g.order,
        });
    }

    out.sort_by(|a, b| a.z.total_cmp(&b.z).then(a.order.cmp(&b.order)));
    out
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3};

    use super::compose_pawn;
    use crate::pawn::model::{
        ApparelLayer, ApparelRenderInput, BeardTypeRenderData, BodyTypeRenderData,
        HeadTypeRenderData, HediffOverlayInput, OverlayAnchor, PawnComposeConfig, PawnDrawFlags,
        PawnFacing, PawnRenderInput,
    };

    fn fixture_input() -> PawnRenderInput {
        PawnRenderInput {
            label: "PawnA".to_string(),
            facing: PawnFacing::South,
            world_pos: Vec3::new(2.5, 3.5, 0.0),
            body_tex_path: "Things/Pawn/Humanlike/Bodies/Naked_Male".to_string(),
            head_tex_path: Some("Things/Pawn/Humanlike/Heads/Male/Average_Normal".to_string()),
            stump_tex_path: Some("Things/Pawn/Humanlike/Heads/Stumps/Stump".to_string()),
            hair_tex_path: Some("Things/Pawn/Humanlike/Hairs/Shaved".to_string()),
            beard_tex_path: Some("Things/Pawn/Humanlike/Beards/Beard_Full".to_string()),
            body_size: Vec2::new(1.4, 1.4),
            head_size: Vec2::new(1.1, 1.1),
            stump_size: Vec2::new(0.8, 0.8),
            hair_size: Vec2::new(1.1, 1.1),
            beard_size: Vec2::new(1.0, 1.0),
            body_type: BodyTypeRenderData {
                head_offset: Vec2::new(0.0, 0.22),
                body_size_factor: 1.0,
            },
            head_type: HeadTypeRenderData::default(),
            beard_type: BeardTypeRenderData::default(),
            tint: [1.0, 1.0, 1.0, 1.0],
            apparel: Vec::new(),
            present_body_part_groups: vec!["UpperHead".to_string(), "Torso".to_string()],
            hediff_overlays: Vec::new(),
            draw_flags: PawnDrawFlags::NONE,
        }
    }

    #[test]
    fn full_head_coverage_hides_hair_and_beard() {
        let mut input = fixture_input();
        input.apparel.push(ApparelRenderInput {
            label: "MarineHelmet".to_string(),
            tex_path: "Things/Apparel/Headgear/MarineHelmet".to_string(),
            layer: ApparelLayer::Overhead,
            render_as_pack: false,
            explicit_skip_hair: false,
            explicit_skip_beard: false,
            has_explicit_skip_flags: false,
            covers_upper_head: false,
            covers_full_head: true,
            draw_offset: Vec2::ZERO,
            draw_scale: Vec2::ONE,
            layer_override: None,
            draw_size: Vec2::new(1.1, 1.1),
            tint: [1.0, 1.0, 1.0, 1.0],
        });

        let result = compose_pawn(&input, &PawnComposeConfig::default());
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Hair")));
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Beard")));
    }

    #[test]
    fn head_position_uses_body_type_offset() {
        let mut input = fixture_input();
        input.body_type.head_offset = Vec2::new(0.0, 0.30);
        let result = compose_pawn(&input, &PawnComposeConfig::default());
        let head = result
            .nodes
            .iter()
            .find(|n| n.id.ends_with("::Head"))
            .expect("head node");
        assert!(head.world_pos.y > input.world_pos.y + 0.25);
    }

    #[test]
    fn apparel_sorted_by_layer_draw_order() {
        let mut input = fixture_input();
        input.apparel = vec![
            ApparelRenderInput {
                label: "Helmet".to_string(),
                tex_path: "Things/Apparel/Headgear/SimpleHelmet".to_string(),
                layer: ApparelLayer::Overhead,
                render_as_pack: false,
                explicit_skip_hair: false,
                explicit_skip_beard: false,
                has_explicit_skip_flags: false,
                covers_upper_head: true,
                covers_full_head: false,
                draw_offset: Vec2::ZERO,
                draw_scale: Vec2::ONE,
                layer_override: None,
                draw_size: Vec2::new(1.1, 1.1),
                tint: [1.0, 1.0, 1.0, 1.0],
            },
            ApparelRenderInput {
                label: "Shirt".to_string(),
                tex_path: "Things/Apparel/Body/Shirt".to_string(),
                layer: ApparelLayer::OnSkin,
                render_as_pack: false,
                explicit_skip_hair: false,
                explicit_skip_beard: false,
                has_explicit_skip_flags: false,
                covers_upper_head: false,
                covers_full_head: false,
                draw_offset: Vec2::ZERO,
                draw_scale: Vec2::ONE,
                layer_override: None,
                draw_size: Vec2::new(1.3, 1.3),
                tint: [1.0, 1.0, 1.0, 1.0],
            },
        ];

        let result = compose_pawn(&input, &PawnComposeConfig::default());
        let mut apparel = result
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, crate::pawn::tree::PawnNodeKind::Apparel));
        let first = apparel.next().expect("at least one apparel node");
        let second = apparel.next().expect("two apparel nodes");
        assert!(first.id.contains("Shirt"));
        assert!(second.id.contains("Helmet"));
    }

    #[test]
    fn hediff_overlay_requires_body_part_group() {
        let mut input = fixture_input();
        input.hediff_overlays.push(HediffOverlayInput {
            label: "EyePatch".to_string(),
            tex_path: "Things/Pawn/Humanlike/Heads/Male/Average_Normal".to_string(),
            anchor: OverlayAnchor::Head,
            layer_offset: 1,
            draw_size: Vec2::new(0.8, 0.8),
            tint: [1.0, 0.6, 0.6, 0.85],
            required_body_part_group: Some("Eyes".to_string()),
            visible_facing: None,
        });

        let result = compose_pawn(&input, &PawnComposeConfig::default());
        assert!(
            result
                .nodes
                .iter()
                .all(|n| !matches!(n.kind, crate::pawn::tree::PawnNodeKind::Hediff))
        );
    }
}
