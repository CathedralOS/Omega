//! Optimizer module role: executable entrance. Canonical logical-spill slot coloring and replay.

use crate::ValidatedLogicalSpillOperations;

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use identity::stack_slot_coloring_identity;
pub use model::*;
pub use validate::validate_stack_slot_coloring;

/// Assign target-neutral, spill-area-relative storage to validated logical spills.
pub fn color_logical_spill_stack_slots(
    source: &ValidatedLogicalSpillOperations,
    policy: StackSlotColoringPolicy,
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedStackSlotColoring, StackSlotColoringError> {
    let plan = compute::compute_stack_slot_coloring(source, policy, budget)?;
    validate_stack_slot_coloring(source, plan)
}
