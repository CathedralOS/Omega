//! Complete stores consume a checked supplier roster at one invocation cut.
//!
//! A concrete entry owns its body summary. A dynamic slot joins every qualified
//! realization in its closed selector roster, remapping only formal coordinates.
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
        let (parameters, effects) =
            if let Some(state) = validation::call_target_state(self.program, call.target_symbol) {
                let Some(machine) = self.program.machine_holding_state(state.symbol) else {
                    return Vec::new();
                };
                let Some(effects) = frames.state_stored_reference_effects(machine, state) else {
                    return Vec::new();
                };
                (self.program.state_parameters(state), effects)
            } else if let Some(slot) = self.program.dynamic_slot_by_symbol(call.target_symbol) {
                let Some(effects) = frames.dynamic_stored_reference_effects(slot.symbol) else {
                    return Vec::new();
                };
                (slot.parameters.as_slice(), effects)
            } else {
                return Vec::new();
            };
        self.capture_stored_suppliers(parameters, &effects, actuals)
    }
}
