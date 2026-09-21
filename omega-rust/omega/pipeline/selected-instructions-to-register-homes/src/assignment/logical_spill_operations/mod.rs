//! Optimizer module role: executable entrance. Logical spill planning and independent replay.
//!
//! `stage_register_allocation`'s runtime-spill recovery sequences this
//! boundary: over the recovery's input facts it plans the store, reload, and
//! operand-rewrite obligations for the first supported active-resident
//! pressure choice, retains the validated output on the produced allocation,
//! and re-derives it during replay. The durable record, canonical identity,
//! and versioned transport live in `register_homes::logical_spill_operations`;
//! computation, validation, and replay stay transform-local.

use crate::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

mod compute;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use model::*;
pub use register_homes::logical_spill_operations::*;
pub use validate::validate_logical_spill_operations;

/// Plan target-neutral storage, store, reload, and operand-rewrite obligations
/// for the first supported active-resident pressure choice.
pub fn plan_logical_spill_operations<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    choices: &ValidatedSpillChoices,
    policy: LogicalSpillOperationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedLogicalSpillOperations, LogicalSpillOperationError> {
    let plan = compute::compute_terminal_logical_spill_operations(
        selected, ranges, legality, choices, policy, budget,
    )?;
    validate_logical_spill_operations(selected, ranges, legality, choices, plan)
}
