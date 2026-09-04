use omega_optimization_core::{
    Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch,
};

use super::super::RegisterAllocationRuleTargetApplicability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationRecoveryRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
}

impl AllocationRecoveryRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }
}

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

/// Compatibility view derived from the descriptor catalog.
pub const ORDERED_ALLOCATION_RECOVERY_RULES: [Optimization; 2] = [
    ALLOCATION_RECOVERY_RULE_CATALOG[0].optimization(),
    ALLOCATION_RECOVERY_RULE_CATALOG[1].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationRecoveryRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
}
