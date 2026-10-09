use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::PreAllocationPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PreAllocationRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: PreAllocationPolicy,
}

impl PreAllocationRuleCatalogPayload {
    /// The pre-allocation rules this exact selection admits.
    pub const fn policy(self) -> PreAllocationPolicy {
        self.policy
    }
}

pub(crate) type PreAllocationRuleCatalogEntry =
    OptimizationCatalogDescriptor<PreAllocationRuleCatalogPayload>;

/// The single pre-allocation enable/order catalog. Descriptor order determines
/// priority in the incremental candidate frontier, including newly enabled
/// earlier families after a commit; it does not require whole-plan rediscovery.
pub(crate) const PRE_ALLOCATION_RULE_CATALOG: [PreAllocationRuleCatalogEntry; 10] = [
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::SAME_BLOCK_COPY_I64_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedRedundantExtensionRemovalV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::REDUNDANT_EXTENSION_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedAddressOffsetFoldV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::ADDRESS_FOLD_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedConstantBooleanFoldV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::CONSTANT_BOOLEAN_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedConstantBranchFoldV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::CONSTANT_BRANCH_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedDeadCompareRemovalV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::DEAD_COMPARE_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedRedundantCompareRemovalV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::REDUNDANT_COMPARE_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedEquivalentCompareRemovalV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::EQUIVALENT_COMPARE_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedCopiedCallOperandFoldV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::COPIED_CALL_OPERAND_V1,
        },
    ),
    PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedStoredLoadForwardingV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: PreAllocationPolicy::STORED_LOAD_FORWARDING_V1,
        },
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreAllocationRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
