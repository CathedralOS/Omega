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

/// Resolve every selected-lowering catalog row in canonical catalog order.
///
/// The returned policy is a derived set of exact catalog payloads, never a
/// separately declared combined rule. Appending a row therefore requires an
/// explicit payload and cannot fall through an old whole-catalog special case.
pub fn resolve_selected_lowering_rules(
    selections: &OptimizationSelections,
) -> Result<(OptimizationSelections, LiteralFoldPolicy), SelectedLoweringRuleCatalogError> {
    let phase = selections.for_phase(OptimizationExecutionPhase::SelectedLowering);
    if phase.is_empty() {
        return Err(SelectedLoweringRuleCatalogError::MissingSelection);
    }
    if let Some(unsupported) = phase.as_slice().iter().find(|selected| {
        !SELECTED_LOWERING_RULE_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **selected)
    }) {
        return Err(SelectedLoweringRuleCatalogError::UnsupportedSelection(
            *unsupported,
        ));
    }
    let policy = SELECTED_LOWERING_RULE_CATALOG
        .iter()
        .filter(|entry| phase.contains(entry.optimization()))
        .fold(LiteralFoldPolicy::empty(), |policy, entry| {
            policy.union(entry.payload().policy())
        });
    Ok((phase, policy))
}
