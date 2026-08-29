//! Allocation-recovery rule entrance.
//!
//! [`ALLOCATION_RECOVERY_RULE_CATALOG`] is the only enable/order declaration
//! for this phase. This entrance admits its current single-rule execution
//! contract; named rule folders own proposal and independent replay.

mod catalog;
pub(crate) mod fixed_view_copy;
pub(crate) mod pressure_rematerialization;

#[cfg(test)]
mod tests;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

pub use catalog::*;
pub use fixed_view_copy::*;
pub use pressure_rematerialization::*;

/// Select the single allocation-recovery rule currently admitted by the
/// physical pipeline. Empty phase selections deliberately return `None`.
pub fn selected_allocation_recovery_rule(
    selections: &OptimizationSelections,
) -> Result<Option<Optimization>, AllocationRecoveryRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    match phase.as_slice() {
        [] => Ok(None),
        [selected]
            if ALLOCATION_RECOVERY_RULE_CATALOG
                .iter()
                .any(|entry| entry.optimization() == *selected) =>
        {
            Ok(Some(*selected))
        }
        [unsupported] => Err(AllocationRecoveryRuleCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition),
    }
}
