//! Optimizer module role: executable entrance. Logical spill planning and independent replay.
//!
//! `stage_register_allocation`'s runtime-spill recovery sequences this
//! boundary: over the recovery's input facts it plans the store, reload, and
//! operand-rewrite obligations for the first supported active-resident
//! pressure choice, retains the validated output on the produced allocation,
//! and re-derives it during replay.

use crate::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

mod codec;
mod compute;
mod identity;
mod model;
mod validate;

#[cfg(test)]
mod tests;

pub use identity::logical_spill_operation_identity;
pub use model::*;
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
