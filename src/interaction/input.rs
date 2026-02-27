use super::InteractionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionAction {
    NoOp,
    HoverChanged(Option<(i32, i32)>),
    SelectCell((i32, i32)),
    SelectPawn { pawn_id: usize, cell: (i32, i32) },
    IssueMove { pawn_id: usize, dest: (i32, i32) },
    ClearSelection,
}

pub fn on_cursor_moved(
    state: &mut InteractionState,
    hovered_cell: Option<(i32, i32)>,
) -> InteractionAction {
    if state.hovered_cell == hovered_cell {
        return InteractionAction::NoOp;
    }
    state.hovered_cell = hovered_cell;
    InteractionAction::HoverChanged(hovered_cell)
}

pub fn on_left_click(
    state: &mut InteractionState,
    pawn_at_hover: Option<usize>,
) -> InteractionAction {
    let Some(cell) = state.hovered_cell else {
        return InteractionAction::NoOp;
    };

    if let Some(pawn_id) = pawn_at_hover {
        state.selected_pawn_id = Some(pawn_id);
        state.selected_cell = Some(cell);
        return InteractionAction::SelectPawn { pawn_id, cell };
    }

    if let Some(selected_pawn_id) = state.selected_pawn_id {
        state.last_issued_destination = Some(cell);
        return InteractionAction::IssueMove {
            pawn_id: selected_pawn_id,
            dest: cell,
        };
    }

    state.selected_cell = Some(cell);
    InteractionAction::SelectCell(cell)
}

pub fn on_right_click(state: &mut InteractionState) -> InteractionAction {
    clear_selection(state)
}

pub fn on_escape(state: &mut InteractionState) -> InteractionAction {
    clear_selection(state)
}

fn clear_selection(state: &mut InteractionState) -> InteractionAction {
    if state.selected_pawn_id.is_none() && state.selected_cell.is_none() {
        return InteractionAction::NoOp;
    }
    state.selected_pawn_id = None;
    state.selected_cell = None;
    InteractionAction::ClearSelection
}

#[cfg(test)]
mod tests {
    use super::{InteractionAction, on_cursor_moved, on_escape, on_left_click, on_right_click};
    use crate::interaction::InteractionState;

    #[test]
    fn left_click_selects_hovered_pawn() {
        let mut state = InteractionState {
            hovered_cell: Some((3, 4)),
            ..Default::default()
        };

        let action = on_left_click(&mut state, Some(7));
        assert_eq!(
            action,
            InteractionAction::SelectPawn {
                pawn_id: 7,
                cell: (3, 4)
            }
        );
        assert_eq!(state.selected_pawn_id, Some(7));
        assert_eq!(state.selected_cell, Some((3, 4)));
    }

    #[test]
    fn left_click_issues_move_for_selected_pawn() {
        let mut state = InteractionState {
            hovered_cell: Some((5, 2)),
            selected_pawn_id: Some(11),
            ..Default::default()
        };

        let action = on_left_click(&mut state, None);
        assert_eq!(
            action,
            InteractionAction::IssueMove {
                pawn_id: 11,
                dest: (5, 2)
            }
        );
        assert_eq!(state.last_issued_destination, Some((5, 2)));
    }

    #[test]
    fn right_click_clears_selection() {
        let mut state = InteractionState {
            selected_pawn_id: Some(2),
            selected_cell: Some((1, 1)),
            ..Default::default()
        };

        let action = on_right_click(&mut state);
        assert_eq!(action, InteractionAction::ClearSelection);
        assert_eq!(state.selected_pawn_id, None);
        assert_eq!(state.selected_cell, None);
    }

    #[test]
    fn escape_clears_selection() {
        let mut state = InteractionState {
            selected_pawn_id: Some(2),
            ..Default::default()
        };

        let action = on_escape(&mut state);
        assert_eq!(action, InteractionAction::ClearSelection);
        assert_eq!(state.selected_pawn_id, None);
    }

    #[test]
    fn cursor_moved_updates_hover_state() {
        let mut state = InteractionState::default();

        let changed = on_cursor_moved(&mut state, Some((0, 0)));
        assert_eq!(changed, InteractionAction::HoverChanged(Some((0, 0))));
        let unchanged = on_cursor_moved(&mut state, Some((0, 0)));
        assert_eq!(unchanged, InteractionAction::NoOp);
    }
}
