use std::collections::HashMap;
use std::time::{Duration, Instant};

use glam::Vec2;

use crate::interaction::InteractionState;
use crate::world::{
    WorldState, issue_move_intent, pawn_id_at_cell, pawn_is_idle, selected_pawn, tick_world,
};

pub mod render_bridge;

#[derive(Debug, Clone, Copy)]
pub struct V2RuntimeConfig {
    pub fixed_dt_seconds: f32,
}

impl Default for V2RuntimeConfig {
    fn default() -> Self {
        Self {
            fixed_dt_seconds: 1.0 / 60.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct V2FrameOutput {
    pub pawn_offsets: HashMap<usize, Vec2>,
    pub hovered_cell: Option<(i32, i32)>,
    pub selected_cell: Option<(i32, i32)>,
    pub selected_world_pos: Option<Vec2>,
    pub selected_path_cells: Vec<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionOutcome {
    NoOp,
    SelectedCell((i32, i32)),
    SelectedPawn { pawn_id: usize, cell: (i32, i32) },
    IssuedMove { pawn_id: usize, dest: (i32, i32) },
    ClearedSelection,
}

#[derive(Debug, Clone)]
pub struct V2Runtime {
    world: WorldState,
    interaction: InteractionState,
    selected_pawn_id: Option<usize>,
    pawn_initial_world_pos: HashMap<usize, Vec2>,
    fixed_dt_seconds: f32,
    step_accumulator: Duration,
    last_step_instant: Option<Instant>,
    frame_count: u64,
    tick_count: u64,
}

impl V2Runtime {
    pub fn new(world: WorldState, config: V2RuntimeConfig) -> Self {
        let pawn_initial_world_pos = world
            .pawns
            .iter()
            .map(|pawn| (pawn.id, pawn.world_pos))
            .collect();
        Self {
            world,
            interaction: InteractionState::default(),
            selected_pawn_id: None,
            pawn_initial_world_pos,
            fixed_dt_seconds: config.fixed_dt_seconds.max(0.0001),
            step_accumulator: Duration::ZERO,
            last_step_instant: None,
            frame_count: 0,
            tick_count: 0,
        }
    }

    pub fn map_bounds(&self) -> (usize, usize) {
        (self.world.width, self.world.height)
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
            self.tick_count += 1;
            tick_world(&mut self.world, self.fixed_dt_seconds);
        }
    }

    pub fn bump_frame_count(&mut self) {
        self.frame_count += 1;
    }

    pub fn on_cursor_cell(&mut self, hovered_cell: Option<(i32, i32)>) -> bool {
        if self.interaction.hovered_cell == hovered_cell {
            return false;
        }
        self.interaction.hovered_cell = hovered_cell;
        true
    }

    pub fn on_left_click(&mut self) -> InteractionOutcome {
        let Some(cell) = self.interaction.hovered_cell else {
            return InteractionOutcome::NoOp;
        };

        if let Some(hit_pawn_id) = pawn_id_at_cell(&self.world, cell) {
            self.selected_pawn_id = Some(hit_pawn_id);
            self.interaction.selected_cell = Some(cell);
            return InteractionOutcome::SelectedPawn {
                pawn_id: hit_pawn_id,
                cell,
            };
        }

        if let Some(selected_pawn_id) = self.selected_pawn_id
            && issue_move_intent(&mut self.world, selected_pawn_id, cell)
        {
            return InteractionOutcome::IssuedMove {
                pawn_id: selected_pawn_id,
                dest: cell,
            };
        }

        self.interaction.selected_cell = Some(cell);
        InteractionOutcome::SelectedCell(cell)
    }

    pub fn clear_selection(&mut self) -> InteractionOutcome {
        let had_selection =
            self.selected_pawn_id.is_some() || self.interaction.selected_cell.is_some();
        self.selected_pawn_id = None;
        self.interaction.selected_cell = None;
        if had_selection {
            InteractionOutcome::ClearedSelection
        } else {
            InteractionOutcome::NoOp
        }
    }

    pub fn selected_pawn_idle(&self) -> Option<bool> {
        self.selected_pawn_id
            .and_then(|id| pawn_is_idle(&self.world, id))
    }

    pub fn frame_output(&self) -> V2FrameOutput {
        let mut pawn_offsets = HashMap::with_capacity(self.world.pawns.len());
        for pawn in &self.world.pawns {
            let initial = self
                .pawn_initial_world_pos
                .get(&pawn.id)
                .copied()
                .unwrap_or(pawn.world_pos);
            pawn_offsets.insert(pawn.id, pawn.world_pos - initial);
        }

        let selected = selected_pawn(&self.world, self.selected_pawn_id);
        V2FrameOutput {
            pawn_offsets,
            hovered_cell: self.interaction.hovered_cell,
            selected_cell: self.interaction.selected_cell,
            selected_world_pos: selected.map(|pawn| pawn.world_pos),
            selected_path_cells: selected
                .map(|pawn| {
                    pawn.path_cells
                        .iter()
                        .skip(pawn.path_index)
                        .copied()
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}
