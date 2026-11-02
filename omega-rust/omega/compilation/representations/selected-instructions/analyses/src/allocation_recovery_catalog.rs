//! The allocation-recovery enable/order catalog. Register allocation (Omega 05)
//! executes these rules; selected optimization (Omega 04) carries the same
//! rows as its stage catalog's allocation-recovery slice so a composition
//! with an earlier phase rejects there.

use optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationExecutionPhase,
    OptimizationPhaseMismatch, OptimizationPhaseSelections,
};

use crate::rewriting::RegisterAllocationRuleTargetApplicability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationRecoveryRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
}

impl AllocationRecoveryRuleCatalogPayload {}

pub type AllocationRecoveryRuleCatalogEntry =
    OptimizationCatalogDescriptor<AllocationRecoveryRuleCatalogPayload>;

/// The single allocation-recovery enable/order catalog.
pub const ALLOCATION_RECOVERY_RULE_CATALOG: [AllocationRecoveryRuleCatalogEntry; 2] = [
    AllocationRecoveryRuleCatalogEntry::new(
        Optimization::SharedEntryFixedViewCopyAfterCompareBeforeBranchV1,
        AllocationRecoveryRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
        },
    ),
    AllocationRecoveryRuleCatalogEntry::new(
        Optimization::ActiveResidentImmediateU64MultiUseRematerializationV1,
        AllocationRecoveryRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
        },
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationRecoveryRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
}

/// Select the single allocation-recovery rule currently admitted by the
/// physical pipeline. Empty phase selections deliberately return `None`.
pub fn selected_allocation_recovery_rule(
    selections: &OptimizationPhaseSelections,
) -> Result<Option<Optimization>, AllocationRecoveryRuleCatalogError> {
    let phase = selections
        .require_phase(OptimizationExecutionPhase::AllocationRecovery)
        .map_err(AllocationRecoveryRuleCatalogError::WrongPhase)?;
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
