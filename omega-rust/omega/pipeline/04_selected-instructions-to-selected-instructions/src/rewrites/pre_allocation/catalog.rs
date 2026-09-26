use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::PreAllocationPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreAllocationRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: PreAllocationPolicy,
}

impl PreAllocationRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }

    /// The pre-allocation rules this exact selection admits.
    pub const fn policy(self) -> PreAllocationPolicy {
        self.policy
    }
}

pub type PreAllocationRuleCatalogEntry =
    OptimizationCatalogDescriptor<PreAllocationRuleCatalogPayload>;

/// The single pre-allocation enable/order catalog. Descriptor order is the
/// deterministic discovery order inside one joint fixed-point sweep: the
/// copy-removal pass scans first, then the extension pass, then the
/// address-fold pass, then the constant-boolean pass, and a commit by any
/// of them restarts the whole sweep over the transformed program.
pub const PRE_ALLOCATION_RULE_CATALOG: [PreAllocationRuleCatalogEntry; 4] = [
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
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAllocationRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
