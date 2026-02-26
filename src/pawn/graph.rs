use super::model::{ApparelRenderInput, HediffOverlayInput};
use super::tree::PawnNodeKind;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AnchorKind {
    Body,
    Head,
}

#[derive(Debug, Clone)]
pub enum NodePayload {
    Body,
    Head,
    Stump,
    Hair,
    Beard,
    Apparel(ApparelRenderInput),
    Hediff(HediffOverlayInput),
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub kind: PawnNodeKind,
    pub anchor: AnchorKind,
    pub order: usize,
    pub payload: NodePayload,
}
