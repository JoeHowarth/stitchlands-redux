use crate::cell::Cell;

#[derive(Debug, Clone, Default)]
pub struct InteractionState {
    pub hovered_cell: Option<Cell>,
    pub selected_cell: Option<Cell>,
    pub selected_pawn_id: Option<usize>,
    pub last_issued_destination: Option<Cell>,
}
