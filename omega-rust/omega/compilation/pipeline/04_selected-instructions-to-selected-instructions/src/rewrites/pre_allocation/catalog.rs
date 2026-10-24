use optimization_core::{Optimization, OptimizationCatalogDescriptor, OptimizationPhaseMismatch};

use super::super::RegisterAllocationRuleTargetApplicability;
use super::PreAllocationPolicy;
use super::execution::{
    PreAllocationCandidateSurface, PreAllocationRuleAdmission, address_fold_candidate,
    constant_boolean_candidate, constant_branch_candidate, copied_call_operand_candidate,
    copy_removal_candidate, dead_compare_candidate, equivalent_compare_candidate,
    extension_removal_candidate, is_boolean_reader, is_branch_reader, is_call_consumer,
    is_compare_publisher, is_copy_i64, is_extension_candidate, is_fold_consumer, is_load_candidate,
    redundant_compare_candidate, stored_load_forwarding_candidate,
};

/// One catalog row's whole declaration: the exact policy a selection enables,
/// its target applicability, and the admission mechanism — candidate surface
/// plus site attempt — the ordered frontier and the fixed-point driver read.
/// A family joins the slice by declaring this row; there is no second
/// dispatch site to edit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PreAllocationRuleCatalogPayload {
    target: RegisterAllocationRuleTargetApplicability,
    policy: PreAllocationPolicy,
    admission: PreAllocationRuleAdmission,
}

impl PreAllocationRuleCatalogPayload {
    /// The pre-allocation rules this exact selection admits.
    pub const fn policy(self) -> PreAllocationPolicy {
        self.policy
    }

    /// The row's candidate surface and site attempt.
    pub const fn admission(&self) -> &PreAllocationRuleAdmission {
        &self.admission
    }
}

pub(crate) type PreAllocationRuleCatalogEntry =
    OptimizationCatalogDescriptor<PreAllocationRuleCatalogPayload>;

/// One row's exact declaration: the policy bit a selection enables and the
/// admission mechanism its family admits sites through.
const fn row(
    optimization: Optimization,
    policy: PreAllocationPolicy,
    admission: PreAllocationRuleAdmission,
) -> PreAllocationRuleCatalogEntry {
    PreAllocationRuleCatalogEntry::new(
        optimization,
        PreAllocationRuleCatalogPayload {
            target: RegisterAllocationRuleTargetApplicability::TargetIndependent,
            policy,
            admission,
        },
    )
}

/// The single pre-allocation enable/order catalog. Descriptor order determines
/// priority in the incremental candidate frontier, including newly enabled
/// earlier families after a commit; it does not require whole-plan rediscovery.
pub(crate) const PRE_ALLOCATION_RULE_CATALOG: [PreAllocationRuleCatalogEntry; 10] = [
    row(
        Optimization::SelectedSameBlockCopyI64RemovalV1,
        PreAllocationPolicy::SAME_BLOCK_COPY_I64_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_copy_i64),
            copy_removal_candidate,
        ),
    ),
    row(
        Optimization::SelectedRedundantExtensionRemovalV1,
        PreAllocationPolicy::REDUNDANT_EXTENSION_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_extension_candidate),
            extension_removal_candidate,
        ),
    ),
    row(
        Optimization::SelectedAddressOffsetFoldV1,
        PreAllocationPolicy::ADDRESS_FOLD_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_fold_consumer),
            address_fold_candidate,
        ),
    ),
    row(
        Optimization::SelectedConstantBooleanFoldV1,
        PreAllocationPolicy::CONSTANT_BOOLEAN_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_boolean_reader),
            constant_boolean_candidate,
        ),
    ),
    row(
        Optimization::SelectedConstantBranchFoldV1,
        PreAllocationPolicy::CONSTANT_BRANCH_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::terminator(is_branch_reader),
            constant_branch_candidate,
        ),
    ),
    row(
        Optimization::SelectedDeadCompareRemovalV1,
        PreAllocationPolicy::DEAD_COMPARE_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_compare_publisher),
            dead_compare_candidate,
        ),
    ),
    row(
        Optimization::SelectedRedundantCompareRemovalV1,
        PreAllocationPolicy::REDUNDANT_COMPARE_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_compare_publisher),
            redundant_compare_candidate,
        ),
    ),
    row(
        Optimization::SelectedEquivalentCompareRemovalV1,
        PreAllocationPolicy::EQUIVALENT_COMPARE_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_compare_publisher),
            equivalent_compare_candidate,
        ),
    ),
    row(
        Optimization::SelectedCopiedCallOperandFoldV1,
        PreAllocationPolicy::COPIED_CALL_OPERAND_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_call_consumer),
            copied_call_operand_candidate,
        ),
    ),
    row(
        Optimization::SelectedStoredLoadForwardingV1,
        PreAllocationPolicy::STORED_LOAD_FORWARDING_V1,
        PreAllocationRuleAdmission::new(
            PreAllocationCandidateSurface::body(is_load_candidate),
            stored_load_forwarding_candidate,
        ),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreAllocationRuleCatalogError {
    WrongPhase(OptimizationPhaseMismatch),
    MissingSelection,
    UnsupportedSelection(Optimization),
}
