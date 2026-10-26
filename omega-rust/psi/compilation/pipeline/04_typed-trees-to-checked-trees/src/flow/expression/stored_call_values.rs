//! Complete stores consume a checked supplier roster at one invocation cut.
//!
//! A concrete entry or authenticated selector occurrence owns its body summary.
//! A visible implementation roster is not source selection evidence.
//! Incomplete suppliers retain their lifetime debts but publish no geometry.
//! Logical postconditions still come only from the public contract.

mod suppliers;

use super::Execution;
use arena::Handle;
use checked_trees::{BorrowCallFact, FlowOperandFact};
use facts::PlaceSegment;
use validation::CanonicalPlace;

impl Execution<'_, '_, '_> {
    pub(super) fn capture_call_stores(
        &mut self,
        call: &BorrowCallFact,
        actuals: &[FlowOperandFact],
    ) -> Vec<(CanonicalPlace, Handle<FlowOperandFact>, Vec<PlaceSegment>)> {
        let Some(frames) = self.context.call_frames else {
            return Vec::new();
        };
        let Some((machine, state, actuals)) = self.selected_store_target(call, actuals) else {
            return Vec::new();
        };
        let Some(effects) = frames.state_stored_reference_effects(machine, state) else {
            return Vec::new();
        };
        self.capture_stored_suppliers(self.program.state_parameters(state), &effects, &actuals)
    }
}
