use std::collections::HashMap;
use std::time::{Duration, Instant};

use glam::{Vec2, Vec3};

use crate::cell::Cell;
use crate::interaction::{
    InteractionAction, InteractionState, on_cursor_moved, on_escape, on_left_click, on_right_click,
};
use crate::pawn::PawnComposeConfig;
#[cfg(test)]
use crate::pawn::compose_pawn;
pub use crate::scene::builder::PawnVisualProfile;
use crate::scene::builder::{OwnedSceneDefs, build_scene};
use crate::scene::{FULL_UV_RECT, MaterialKind, Scene, SpriteParams, SpriteRecord, TextureHandle};
use crate::world::{
    WorldState, issue_move_intent, pawn_id_at_cell, pawn_is_idle, selected_pawn, tick_world,
};

/// Append interaction-driven overlay markers (selected-path trail, hover
/// highlight, selection ring) on top of the per-frame dynamic sprite list.
/// The base list comes from `V2Runtime::build_scene`; this is the seam where
/// non-world UI markers attach.
pub fn apply_interaction_overlays(
    base_dynamic: &[SpriteRecord],
    overlay_texture: TextureHandle,
    frame: &V2FrameOutput,
) -> Vec<SpriteRecord> {
    let mut out = base_dynamic.to_vec();

    for cell in &frame.selected_path_cells {
        out.push(SpriteRecord {
            texture: overlay_texture,
            params: SpriteParams {
                world_pos: Vec3::new(cell.x as f32 + 0.5, cell.z as f32 + 0.5, -0.23),
                size: Vec2::new(0.36, 0.36),
                tint: [0.35, 1.00, 0.45, 0.65],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    }

    if let Some(Cell { x, z }) = frame.hovered_cell {
        out.push(SpriteRecord {
            texture: overlay_texture,
            params: SpriteParams {
                world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.22),
                size: Vec2::new(1.04, 1.04),
                tint: [0.20, 0.90, 1.00, 0.28],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    }

    if let Some(selected_pos) = frame.selected_world_pos {
        out.push(SpriteRecord {
            texture: overlay_texture,
            params: SpriteParams {
                world_pos: Vec3::new(selected_pos.x, selected_pos.y, -0.21),
                size: Vec2::new(1.16, 1.16),
                tint: [1.00, 0.90, 0.20, 0.30],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    } else if let Some(Cell { x, z }) = frame.selected_cell {
        out.push(SpriteRecord {
            texture: overlay_texture,
            params: SpriteParams {
                world_pos: Vec3::new(x as f32 + 0.5, z as f32 + 0.5, -0.21),
                size: Vec2::new(1.10, 1.10),
                tint: [1.00, 0.90, 0.20, 0.30],
                uv_rect: FULL_UV_RECT,
            },
            material: MaterialKind::SolidColor,
        });
    }

    out
}

#[derive(Debug, Clone)]
pub struct V2RuntimeConfig {
    pub fixed_dt_seconds: f32,
    pub compose_config: PawnComposeConfig,
    pub scene_defs: Option<OwnedSceneDefs>,
    pub pawn_profiles: HashMap<usize, PawnVisualProfile>,
}

impl Default for V2RuntimeConfig {
    fn default() -> Self {
        Self {
            fixed_dt_seconds: 1.0 / 60.0,
            compose_config: PawnComposeConfig::default(),
            scene_defs: None,
            pawn_profiles: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct V2FrameOutput {
    pub hovered_cell: Option<Cell>,
    pub selected_cell: Option<Cell>,
    pub selected_world_pos: Option<Vec2>,
    pub selected_path_cells: Vec<Cell>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionOutcome {
    NoOp,
    SelectedCell(Cell),
    SelectedPawn { pawn_id: usize, cell: Cell },
    IssuedMove { pawn_id: usize, dest: Cell },
    ClearedSelection,
}

#[derive(Debug, Clone)]
pub struct V2Runtime {
    world: WorldState,
    interaction: InteractionState,
    pawn_profiles: HashMap<usize, PawnVisualProfile>,
    scene_defs: Option<OwnedSceneDefs>,
    compose_config: PawnComposeConfig,
    fixed_dt_seconds: f32,
    step_accumulator: Duration,
    last_step_instant: Option<Instant>,
    frame_count: u64,
    tick_count: u64,
}

impl V2Runtime {
    pub fn new(world: WorldState, config: V2RuntimeConfig) -> Self {
        Self {
            world,
            interaction: InteractionState::default(),
            pawn_profiles: config.pawn_profiles,
            scene_defs: config.scene_defs,
            compose_config: config.compose_config,
            fixed_dt_seconds: config.fixed_dt_seconds.max(0.0001),
            step_accumulator: Duration::ZERO,
            last_step_instant: None,
            frame_count: 0,
            tick_count: 0,
        }
    }

    pub fn map_bounds(&self) -> (usize, usize) {
        (self.world.width(), self.world.height())
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    pub fn run_fixed_step(&mut self) {
        let now = Instant::now();
        let previous = self.last_step_instant.unwrap_or(now);
        self.last_step_instant = Some(now);
        self.step_accumulator += now.saturating_duration_since(previous);

        let fixed_dt = Duration::from_secs_f32(self.fixed_dt_seconds);
        while self.step_accumulator >= fixed_dt {
            self.step_accumulator -= fixed_dt;
            self.tick_once();
        }
    }

    pub fn tick_once(&mut self) {
        self.tick_count += 1;
        tick_world(&mut self.world, self.fixed_dt_seconds);
    }

    pub fn bump_frame_count(&mut self) {
        self.frame_count += 1;
    }

    pub fn on_cursor_cell(&mut self, hovered_cell: Option<Cell>) -> bool {
        matches!(
            on_cursor_moved(&mut self.interaction, hovered_cell),
            InteractionAction::HoverChanged(_)
        )
    }

    pub fn on_left_click(&mut self) -> InteractionOutcome {
        let pawn_at_hover = self
            .interaction
            .hovered_cell
            .and_then(|cell| pawn_id_at_cell(&self.world, cell));

        match on_left_click(&mut self.interaction, pawn_at_hover) {
            InteractionAction::NoOp | InteractionAction::HoverChanged(_) => {
                InteractionOutcome::NoOp
            }
            InteractionAction::SelectCell(cell) => InteractionOutcome::SelectedCell(cell),
            InteractionAction::SelectPawn { pawn_id, cell } => {
                InteractionOutcome::SelectedPawn { pawn_id, cell }
            }
            InteractionAction::IssueMove { pawn_id, dest } => {
                if issue_move_intent(&mut self.world, pawn_id, dest) {
                    InteractionOutcome::IssuedMove { pawn_id, dest }
                } else {
                    InteractionOutcome::NoOp
                }
            }
            InteractionAction::ClearSelection => InteractionOutcome::ClearedSelection,
        }
    }

    pub fn on_right_click(&mut self) -> InteractionOutcome {
        match on_right_click(&mut self.interaction) {
            InteractionAction::ClearSelection => InteractionOutcome::ClearedSelection,
            InteractionAction::NoOp
            | InteractionAction::HoverChanged(_)
            | InteractionAction::SelectCell(_)
            | InteractionAction::SelectPawn { .. }
            | InteractionAction::IssueMove { .. } => InteractionOutcome::NoOp,
        }
    }

    pub fn on_escape(&mut self) -> InteractionOutcome {
        match on_escape(&mut self.interaction) {
            InteractionAction::ClearSelection => InteractionOutcome::ClearedSelection,
            InteractionAction::NoOp
            | InteractionAction::HoverChanged(_)
            | InteractionAction::SelectCell(_)
            | InteractionAction::SelectPawn { .. }
            | InteractionAction::IssueMove { .. } => InteractionOutcome::NoOp,
        }
    }

    pub fn selected_pawn_idle(&self) -> Option<bool> {
        self.interaction
            .selected_pawn_id
            .and_then(|id| pawn_is_idle(&self.world, id))
    }

    /// Count of pawn-node sprites this runtime would emit for the current
    /// world snapshot, computed by running the same compose pipeline that
    /// `build_scene` uses but without producing the rest of the scene.
    /// Lets tests assert pawn rendering survives a code change without
    /// needing an `AssetResolver` or a GPU.
    #[cfg(test)]
    pub fn pawn_node_count(&self) -> usize {
        let mut total = 0;
        for pawn in self.world.pawns() {
            let Some(profile) = self.pawn_profiles.get(&pawn.id) else {
                continue;
            };
            let mut render_input = profile.base_render_input.clone();
            render_input.facing = pawn.facing;
            render_input.world_pos = Vec3::new(pawn.world_pos.x, pawn.world_pos.y, 0.0);
            let composed = compose_pawn(&render_input, &self.compose_config);
            total += composed.nodes.len();
        }
        total
    }

    pub fn build_scene(&self) -> anyhow::Result<Scene> {
        let scene_defs = self
            .scene_defs
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("runtime scene defs are not configured"))?;
        build_scene(
            &scene_defs.as_refs(),
            &self.world,
            &self.pawn_profiles,
            &self.compose_config,
        )
    }

    pub fn frame_output(&self) -> V2FrameOutput {
        let selected = selected_pawn(&self.world, self.interaction.selected_pawn_id);
        V2FrameOutput {
            hovered_cell: self.interaction.hovered_cell,
            selected_cell: self.interaction.selected_cell,
            selected_world_pos: selected.map(|pawn| pawn.world_pos),
            selected_path_cells: selected
                .map(|pawn| pawn.path.remaining_cells().to_vec())
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use crate::cell::Cell;
    use crate::fixtures::{MapSpec, PawnSpawn, RenderSpec, SceneFixture, TerrainCell, ThingSpawn};
    use crate::pawn::{
        BeardTypeRenderData, BodyTypeRenderData, HeadTypeRenderData, PawnDrawFlags, PawnFacing,
        PawnRenderInput,
    };
    use crate::world::world_from_fixture;

    use super::{InteractionOutcome, PawnVisualProfile, V2Runtime, V2RuntimeConfig};

    fn profile_for(pawn_id: usize, label: &str) -> PawnVisualProfile {
        PawnVisualProfile {
            pawn_id,
            base_render_input: PawnRenderInput {
                label: label.to_string(),
                facing: PawnFacing::South,
                world_pos: Vec3::new(0.5, 0.5, 0.0),
                body_tex_path: "Things/Pawn/Humanlike/Bodies/Naked_Male".to_string(),
                head_tex_path: Some("Things/Pawn/Humanlike/Heads/Male/Average_Normal".to_string()),
                stump_tex_path: None,
                hair_tex_path: None,
                beard_tex_path: None,
                body_type: BodyTypeRenderData::default(),
                head_type: HeadTypeRenderData::default(),
                beard_type: BeardTypeRenderData::default(),
                tint: [1.0, 1.0, 1.0, 1.0],
                apparel: Vec::new(),
                draw_flags: PawnDrawFlags::NONE,
            },
        }
    }

    fn runtime_for_fixture(fixture: SceneFixture) -> V2Runtime {
        let world = world_from_fixture(&fixture);
        let pawn_profiles = world
            .pawns()
            .iter()
            .map(|pawn| (pawn.id, profile_for(pawn.id, &pawn.label)))
            .collect();
        V2Runtime::new(
            world,
            V2RuntimeConfig {
                pawn_profiles,
                ..V2RuntimeConfig::default()
            },
        )
    }

    fn open_world_fixture() -> SceneFixture {
        SceneFixture {
            schema_version: 2,
            map: MapSpec {
                width: 8,
                height: 6,
                terrain: vec![
                    TerrainCell {
                        terrain_def: "Soil".to_string(),
                    };
                    8 * 6
                ],
                roofs: Vec::new(),
                fog: Vec::new(),
                snow_depth: Vec::new(),
            },
            render: RenderSpec::default(),
            things: Vec::new(),
            pawns: vec![PawnSpawn {
                cell_x: 1,
                cell_z: 1,
                label: Some("PawnA".to_string()),
                body: None,
                head: None,
                hair: None,
                beard: None,
                apparel_defs: Vec::new(),
                facing: PawnFacing::South,
            }],
            camera: None,
        }
    }

    #[test]
    fn select_then_move_issues_path() {
        let mut runtime = runtime_for_fixture(open_world_fixture());

        assert!(runtime.on_cursor_cell(Some(Cell::new(1, 1))));
        assert_eq!(
            runtime.on_left_click(),
            InteractionOutcome::SelectedPawn {
                pawn_id: 0,
                cell: Cell::new(1, 1)
            }
        );
        assert!(runtime.on_cursor_cell(Some(Cell::new(5, 4))));
        assert_eq!(
            runtime.on_left_click(),
            InteractionOutcome::IssuedMove {
                pawn_id: 0,
                dest: Cell::new(5, 4)
            }
        );

        let frame = runtime.frame_output();
        assert!(!frame.selected_path_cells.is_empty());
        assert!(
            runtime.pawn_node_count() > 0,
            "scene assembly must produce at least one pawn-node sprite for a fixture with pawns"
        );
    }

    #[test]
    fn blocked_destination_returns_noop() {
        let mut fixture = open_world_fixture();
        fixture.things.push(ThingSpawn {
            def_name: "ChunkSlagSteel".to_string(),
            cell_x: 5,
            cell_z: 4,
            blocks_movement: true,
        });
        let mut runtime = runtime_for_fixture(fixture);

        assert!(runtime.on_cursor_cell(Some(Cell::new(1, 1))));
        let _ = runtime.on_left_click();
        assert!(runtime.on_cursor_cell(Some(Cell::new(5, 4))));
        assert_eq!(runtime.on_left_click(), InteractionOutcome::NoOp);
    }

    #[test]
    fn pawn_reaches_destination_within_bounded_ticks() {
        let mut runtime = runtime_for_fixture(open_world_fixture());

        assert!(runtime.on_cursor_cell(Some(Cell::new(1, 1))));
        let _ = runtime.on_left_click();
        assert!(runtime.on_cursor_cell(Some(Cell::new(5, 4))));
        let issued = runtime.on_left_click();
        assert!(matches!(issued, InteractionOutcome::IssuedMove { .. }));

        for _ in 0..300 {
            runtime.tick_once();
        }

        let frame = runtime.frame_output();
        let pos = frame
            .selected_world_pos
            .expect("selected pawn should still be available");
        assert!((pos.x - 5.5).abs() < 0.01);
        assert!((pos.y - 4.5).abs() < 0.01);
    }

    #[test]
    fn right_click_and_escape_clear_selection() {
        let mut runtime = runtime_for_fixture(open_world_fixture());

        assert!(runtime.on_cursor_cell(Some(Cell::new(1, 1))));
        let _ = runtime.on_left_click();
        assert_eq!(
            runtime.on_right_click(),
            InteractionOutcome::ClearedSelection
        );
        assert_eq!(runtime.frame_output().selected_world_pos, None);

        let _ = runtime.on_cursor_cell(Some(Cell::new(1, 1)));
        let _ = runtime.on_left_click();
        assert_eq!(runtime.on_escape(), InteractionOutcome::ClearedSelection);
        assert_eq!(runtime.frame_output().selected_world_pos, None);
    }
}
