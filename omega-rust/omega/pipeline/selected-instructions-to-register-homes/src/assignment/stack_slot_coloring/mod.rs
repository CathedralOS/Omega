//! Optimizer module role: executable entrance. Canonical logical-spill slot coloring and replay.
//!
//! `stage_register_allocation`'s runtime-spill recovery sequences this
//! boundary: it colors the retained logical-operation plan and replays the
//! coloring. The durable record, canonical identity, and versioned transport
//! live in `register_homes::stack_slot_coloring`; computation, validation, and
//! replay stay transform-local.

use crate::ValidatedLogicalSpillOperations;

mod compute;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use model::*;
pub use register_homes::stack_slot_coloring::*;
pub use validate::validate_stack_slot_coloring;

/// Assign target-neutral, spill-area-relative storage to validated logical spills.
pub fn color_logical_spill_stack_slots(
    source: &ValidatedLogicalSpillOperations,
    policy: StackSlotColoringPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedStackSlotColoring, StackSlotColoringError> {
    let plan = compute::compute_stack_slot_coloring(source, policy, budget)?;
    validate_stack_slot_coloring(source, plan)
}
