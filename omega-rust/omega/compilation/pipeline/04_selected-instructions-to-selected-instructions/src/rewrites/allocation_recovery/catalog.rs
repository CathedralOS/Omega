use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AllocationRecoveryRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
}

impl AllocationRecoveryRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }
}

pub(crate) type AllocationRecoveryRuleCatalogEntry =
    OptimizationCatalogDescriptor<AllocationRecoveryRuleCatalogPayload>;

/// The single allocation-recovery enable/order catalog.
pub(crate) const ALLOCATION_RECOVERY_RULE_CATALOG: [AllocationRecoveryRuleCatalogEntry; 2] = [
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
