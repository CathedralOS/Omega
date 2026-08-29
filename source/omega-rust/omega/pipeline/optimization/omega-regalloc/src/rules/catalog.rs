use omega_optimization_core::{Optimization, OptimizationCatalogDescriptor};

use super::LiteralFoldPolicy;

/// Register-allocation catalog entries are architecture-independent. The
/// explicit marker keeps that policy in the owning declaration instead of
/// making portability an undocumented default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAllocationRuleTargetApplicability {
    TargetIndependent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocationRecoveryRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
}

impl AllocationRecoveryRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedLoweringRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: LiteralFoldPolicy,
}

impl SelectedLoweringRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }

    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }
}

pub type AllocationRecoveryRuleCatalogEntry =
    OptimizationCatalogDescriptor<AllocationRecoveryRuleCatalogPayload>;
pub type SelectedLoweringRuleCatalogEntry =
    OptimizationCatalogDescriptor<SelectedLoweringRuleCatalogPayload>;

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

/// The single selected-lowering enable/order catalog.
pub const SELECTED_LOWERING_RULE_CATALOG: [SelectedLoweringRuleCatalogEntry; 2] = [
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactAddImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
        },
    ),
    SelectedLoweringRuleCatalogEntry::new(
        Optimization::SelectedIncomingU12ExactSubtractImmediate,
        SelectedLoweringRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: LiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1,
        },
    ),
];

/// Compatibility views derived from the descriptor catalogs, never parallel
/// sources of truth.
pub const ORDERED_ALLOCATION_RECOVERY_RULES: [Optimization; 2] = [
    ALLOCATION_RECOVERY_RULE_CATALOG[0].optimization(),
    ALLOCATION_RECOVERY_RULE_CATALOG[1].optimization(),
];
pub const ORDERED_SELECTED_LOWERING_RULES: [Optimization; 2] = [
    SELECTED_LOWERING_RULE_CATALOG[0].optimization(),
    SELECTED_LOWERING_RULE_CATALOG[1].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationRecoveryRuleCatalogError {
    UnsupportedSelection(Optimization),
    UnsupportedComposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedLoweringRuleCatalogError {
    MissingSelection,
    UnsupportedSelection(Optimization),
}
