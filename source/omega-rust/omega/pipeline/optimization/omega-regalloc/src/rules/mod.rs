//! Register-allocation and selected-lowering rule entrance.
//!
//! [`ORDERED_ALLOCATION_RECOVERY_RULES`] and
//! [`ORDERED_SELECTED_LOWERING_RULES`] are the only enable/order tables for
//! their exact phases. This entrance projects selections through them, then
//! named leaves own proposal and independent replay.

pub(crate) mod fixed_view_copy;
pub(crate) mod literal_fold;
pub(crate) mod pressure_rematerialization;

mod catalog;

#[cfg(test)]
mod tests;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

pub use catalog::{
    AllocationRecoveryRuleCatalogError, ORDERED_ALLOCATION_RECOVERY_RULES,
    ORDERED_SELECTED_LOWERING_RULES, SelectedLoweringRuleCatalogError,
};
pub use fixed_view_copy::*;
pub use literal_fold::*;
pub use pressure_rematerialization::*;

/// Select the single allocation-recovery rule currently admitted by the
/// physical pipeline. Empty phase selections deliberately return `None`.
pub fn selected_allocation_recovery_rule(
    selections: &OptimizationSelections,
) -> Result<Option<Optimization>, AllocationRecoveryRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::AllocationRecovery);
    match phase.as_slice() {
        [] => Ok(None),
        [selected] if ORDERED_ALLOCATION_RECOVERY_RULES.contains(selected) => Ok(Some(*selected)),
        [unsupported] => Err(AllocationRecoveryRuleCatalogError::UnsupportedSelection(
            *unsupported,
        )),
        _ => Err(AllocationRecoveryRuleCatalogError::UnsupportedComposition),
    }
}

/// Select the exact selected-lowering phase and its literal-fold policy.
pub fn selected_lowering_rule_policy(
    selections: &OptimizationSelections,
) -> Result<(OptimizationSelections, LiteralFoldPolicy), SelectedLoweringRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let policy = match phase.as_slice() {
        [Optimization::SelectedIncomingU12ExactAddImmediate] => {
            LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1
        }
        [Optimization::SelectedIncomingU12ExactSubtractImmediate] => {
            LiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1
        }
        [
            Optimization::SelectedIncomingU12ExactAddImmediate,
            Optimization::SelectedIncomingU12ExactSubtractImmediate,
        ] => LiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1,
        [] => return Err(SelectedLoweringRuleCatalogError::MissingSelection),
        selected => {
            return Err(SelectedLoweringRuleCatalogError::UnsupportedSelection(
                selected[0],
            ));
        }
    };
    Ok((phase, policy))
}
