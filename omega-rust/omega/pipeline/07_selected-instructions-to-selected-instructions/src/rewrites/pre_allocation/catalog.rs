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
/// copy-removal pass scans first, then the extension pass, and a commit by
/// either restarts the whole sweep over the transformed program.
pub const PRE_ALLOCATION_RULE_CATALOG: [PreAllocationRuleCatalogEntry; 2] = [
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
];

/// Compatibility view derived from the descriptor catalog.
pub const ORDERED_PRE_ALLOCATION_RULES: [Optimization; 2] = [
    PRE_ALLOCATION_RULE_CATALOG[0].optimization(),
    PRE_ALLOCATION_RULE_CATALOG[1].optimization(),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAllocationRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
