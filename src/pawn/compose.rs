use glam::Vec3;

use super::layering::{apparel_z, facing_x_offset};
use super::model::{ApparelLayer, PawnComposeConfig, PawnRenderInput};
use super::rules::resolve_skip_flags;
use super::tree::{PawnNode, PawnNodeKind};

#[derive(Debug, Clone)]
pub struct PawnComposition {
    pub nodes: Vec<PawnNode>,
}

pub fn compose_pawn(input: &PawnRenderInput, config: &PawnComposeConfig) -> PawnComposition {
    let skip = resolve_skip_flags(input.draw_flags, &input.apparel);
    let x_offset = facing_x_offset(input.facing);
    let base_pos = Vec3::new(
        input.world_pos.x + x_offset,
        input.world_pos.y,
        input.world_pos.z,
    );

    let mut nodes = Vec::new();
    let mut order = 0usize;

    nodes.push(PawnNode {
        id: format!("{}::Body", input.label),
        kind: PawnNodeKind::Body,
        tex_path: input.body_tex_path.clone(),
        world_pos: Vec3::new(base_pos.x, base_pos.y, config.layering.body_z),
        size: input.body_size,
        tint: input.tint,
        z: config.layering.body_z,
        order,
    });
    order += 1;

    if !input.draw_flags.hide_head && !input.draw_flags.head_stump {
        if let Some(tex_path) = &input.head_tex_path {
            nodes.push(PawnNode {
                id: format!("{}::Head", input.label),
                kind: PawnNodeKind::Head,
                tex_path: tex_path.clone(),
                world_pos: Vec3::new(base_pos.x, base_pos.y, config.layering.head_z),
                size: input.head_size,
                tint: input.tint,
                z: config.layering.head_z,
                order,
            });
            order += 1;
        }

        if !skip.hide_hair
            && let Some(tex_path) = &input.hair_tex_path
        {
            nodes.push(PawnNode {
                id: format!("{}::Hair", input.label),
                kind: PawnNodeKind::Hair,
                tex_path: tex_path.clone(),
                world_pos: Vec3::new(base_pos.x, base_pos.y, config.layering.hair_z),
                size: input.hair_size,
                tint: input.tint,
                z: config.layering.hair_z,
                order,
            });
            order += 1;
        }

        if !skip.hide_beard
            && let Some(tex_path) = &input.beard_tex_path
        {
            nodes.push(PawnNode {
                id: format!("{}::Beard", input.label),
                kind: PawnNodeKind::Beard,
                tex_path: tex_path.clone(),
                world_pos: Vec3::new(base_pos.x, base_pos.y, config.layering.beard_z),
                size: input.beard_size,
                tint: input.tint,
                z: config.layering.beard_z,
                order,
            });
            order += 1;
        }
    }

    for (stack_index, apparel) in input.apparel.iter().enumerate() {
        debug_assert!(ApparelLayer::ALL.contains(&apparel.layer));
        let z = apparel_z(config.layering, apparel.layer, stack_index);
        nodes.push(PawnNode {
            id: format!("{}::Apparel::{}", input.label, apparel.label),
            kind: PawnNodeKind::Apparel,
            tex_path: apparel.tex_path.clone(),
            world_pos: Vec3::new(base_pos.x, base_pos.y, z),
            size: apparel.draw_size,
            tint: apparel.tint,
            z,
            order,
        });
        order += 1;
    }

    nodes.sort_by(|a, b| a.z.total_cmp(&b.z).then(a.order.cmp(&b.order)));
    PawnComposition { nodes }
}

#[cfg(test)]
mod tests {
    use glam::{Vec2, Vec3};

    use super::compose_pawn;
    use crate::pawn::model::{
        ApparelLayer, ApparelRenderInput, PawnComposeConfig, PawnDrawFlags, PawnFacing,
        PawnRenderInput,
    };

    fn fixture_input() -> PawnRenderInput {
        PawnRenderInput {
            label: "PawnA".to_string(),
            facing: PawnFacing::South,
            world_pos: Vec3::new(2.5, 3.5, 0.0),
            body_tex_path: "Things/Pawn/Humanlike/Bodies/Naked_Male".to_string(),
            head_tex_path: Some("Things/Pawn/Humanlike/Heads/Male/Average_Normal".to_string()),
            hair_tex_path: Some("Things/Pawn/Humanlike/Hairs/Shaved".to_string()),
            beard_tex_path: Some("Things/Pawn/Humanlike/Beards/Beard_Full".to_string()),
            body_size: Vec2::new(1.4, 1.4),
            head_size: Vec2::new(1.1, 1.1),
            hair_size: Vec2::new(1.1, 1.1),
            beard_size: Vec2::new(1.0, 1.0),
            tint: [1.0, 1.0, 1.0, 1.0],
            apparel: Vec::new(),
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
            covers_upper_head: false,
            covers_full_head: true,
            draw_size: Vec2::new(1.1, 1.1),
            tint: [1.0, 1.0, 1.0, 1.0],
        });

        let result = compose_pawn(&input, &PawnComposeConfig::default());
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Hair")));
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Beard")));
    }

    #[test]
    fn layer_order_is_deterministic() {
        let mut input = fixture_input();
        input.apparel.push(ApparelRenderInput {
            label: "Jacket".to_string(),
            tex_path: "Things/Apparel/Body/Jacket".to_string(),
            layer: ApparelLayer::Shell,
            covers_upper_head: false,
            covers_full_head: false,
            draw_size: Vec2::new(1.5, 1.5),
            tint: [1.0, 1.0, 1.0, 1.0],
        });

        let result = compose_pawn(&input, &PawnComposeConfig::default());
        let mut previous = f32::NEG_INFINITY;
        for node in result.nodes {
            assert!(node.z >= previous);
            previous = node.z;
        }
    }

    #[test]
    fn head_hide_flag_removes_head_hair_beard() {
        let mut input = fixture_input();
        input.draw_flags = PawnDrawFlags {
            hide_hair: false,
            hide_beard: false,
            hide_head: true,
            head_stump: false,
        };

        let result = compose_pawn(&input, &PawnComposeConfig::default());
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Head")));
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Hair")));
        assert!(result.nodes.iter().all(|n| !n.id.ends_with("::Beard")));
    }
}
