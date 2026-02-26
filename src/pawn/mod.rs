pub mod compose;
pub mod layering;
pub mod model;
pub mod rules;
pub mod tree;

pub use compose::compose_pawn;
pub use model::{
    ApparelLayer, ApparelRenderInput, HediffOverlayInput, OverlayAnchor, PawnComposeConfig,
    PawnDrawFlags, PawnFacing, PawnRenderInput,
};
