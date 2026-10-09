//! Independent validation of store mutation motion.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! motion's legality from the source records — the named instruction's
//! store kind against its single write row, the byte extent the row's
//! place and range spell (or the fixed byte a resolved sequence index
//! lands it on), the operand surface the route declares, the forward walk
//! that lands the store immediately before the first position that must
//! stay ordered after it, the crossed-edge audits that keep the carried
//! registers and the moved storage untouched, and the boundary-settlement
//! remap the landing demands — then rebuilds the function the contract
//! demands and requires the proposal to equal it. Moving the store back
//! must reproduce the complete source by content. A producer admission
//! error therefore fails validation even when the proposal is exactly
//! what that producer emitted.
use selected_instructions::SelectedInstructionPlan;
