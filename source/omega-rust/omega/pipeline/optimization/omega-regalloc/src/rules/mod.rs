//! Register-allocation and selected-lowering rule entrance.
//!
//! [`ALLOCATION_RECOVERY_RULE_CATALOG`] and
//! [`SELECTED_LOWERING_RULE_CATALOG`] are the only enable/order declarations
//! for their exact phases. This entrance projects selections through them;
//! named leaf folders own proposal and independent replay.

pub(crate) mod fixed_view_copy;
pub(crate) mod literal_fold;
pub(crate) mod pressure_rematerialization;

mod catalog;

#[cfg(test)]
mod tests;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

pub use catalog::{
    ALLOCATION_RECOVERY_RULE_CATALOG, AllocationRecoveryRuleCatalogEntry,
    AllocationRecoveryRuleCatalogError, AllocationRecoveryRuleCatalogPayload,
    ORDERED_ALLOCATION_RECOVERY_RULES, ORDERED_SELECTED_LOWERING_RULES,
    RegisterAllocationRuleTargetApplicability, SELECTED_LOWERING_RULE_CATALOG,
    SelectedLoweringRuleCatalogEntry, SelectedLoweringRuleCatalogError,
    SelectedLoweringRuleCatalogPayload,
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

/// Select the exact selected-lowering phase and its literal-fold policy.
pub fn selected_lowering_rule_policy(
    selections: &OptimizationSelections,
) -> Result<(OptimizationSelections, LiteralFoldPolicy), SelectedLoweringRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    let policy = match phase.as_slice() {
        [] => return Err(SelectedLoweringRuleCatalogError::MissingSelection),
        [selected] => SELECTED_LOWERING_RULE_CATALOG
            .iter()
            .find(|entry| entry.optimization() == *selected)
            .map(|entry| entry.payload().policy())
            .ok_or(SelectedLoweringRuleCatalogError::UnsupportedSelection(
                *selected,
            ))?,
        selected if selected == ORDERED_SELECTED_LOWERING_RULES => {
            LiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1
        }
        selected => {
            return Err(SelectedLoweringRuleCatalogError::UnsupportedSelection(
                selected[0],
            ));
        }
    };
    Ok((phase, policy))
}
