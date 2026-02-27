#[derive(Debug, Clone, Default)]
pub struct InteractionState {
    pub hovered_cell: Option<(i32, i32)>,
    pub selected_cell: Option<(i32, i32)>,
    pub selected_pawn_id: Option<usize>,
    pub last_issued_destination: Option<(i32, i32)>,
}
