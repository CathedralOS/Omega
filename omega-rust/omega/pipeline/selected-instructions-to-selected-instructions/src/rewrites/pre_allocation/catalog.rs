use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::CopyRemovalPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreAllocationRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: CopyRemovalPolicy,
}

impl PreAllocationRuleCatalogPayload {
    pub const fn target(self) -> RegisterAllocationRuleTargetApplicability {
        self.target
    }

    /// The copy-removal rules this exact selection admits.
    pub const fn policy(self) -> CopyRemovalPolicy {
        self.policy
    }
}

pub type PreAllocationRuleCatalogEntry =
    OptimizationCatalogDescriptor<PreAllocationRuleCatalogPayload>;

/// The single pre-allocation enable/order catalog.
pub const PRE_ALLOCATION_RULE_CATALOG: [PreAllocationRuleCatalogEntry; 1] =
    [PreAllocationRuleCatalogEntry::new(
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy: CopyRemovalPolicy::SAME_BLOCK_COPY_I64_V1,
        },
    )];

/// Compatibility view derived from the descriptor catalog.
pub const ORDERED_PRE_ALLOCATION_RULES: [Optimization; 1] =
    [PRE_ALLOCATION_RULE_CATALOG[0].optimization()];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreAllocationRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
