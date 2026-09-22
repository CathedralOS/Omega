//! Optimizer module role: executable entrance. Built-in Psi optimization stage entrance.
//!
//! [`PSI_PASS_CATALOG`] is the only enable/disable and pass-order table. This
//! entrance filters exact selections through it and returns one ordered
//! registry per selected pass. Each pass is one file beside this one --
//! `<pass>.rs` holds that pass's visible local rule order and the contracts
//! its rules share -- and its rules are the files under `<pass>/`, one per
//! rule (or one table per rule family over a single proposal). `support`
//! holds the evidence and control-flow mechanics several passes share.
//! Independent acceptance remains in `optimization-unit-semantics`.

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v5";
const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v13";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";
const DEAD_PURE_SCALAR_PASS_NAME: &[u8] = b"omega.psi-pass.dead-pure-scalar-elimination.v2";
const PROOF_CHECK_ELISION_PASS_NAME: &[u8] = b"omega.psi-pass.proof-check-elision.v12";
const GLOBAL_VALUE_NUMBERING_PASS_NAME: &[u8] = b"omega.psi-pass.global-value-numbering.v14";
const STATE_SPECIALIZATION_PASS_NAME: &[u8] = b"omega.psi-pass.state-specialization.v1";
const REPRESENTATION_SPECIALIZATION_PASS_NAME: &[u8] =
    b"omega.psi-pass.representation-specialization.v1";

mod catalog;
pub(crate) mod registry;
mod support;

mod case_membership_specialization;
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod field_value_specialization;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;
mod state_specialization;

/// The representation-specialization pass runs both registered families in
/// schedule order: case-membership folds (ordinal 0), then field-value folds
/// (ordinal 1). Both are published under the same public selection.
fn representation_specialization_rule_registrations() -> Vec<catalog::BuiltInRuleRegistration> {
    let mut registrations = case_membership_specialization::built_in_registrations();
    registrations.extend(field_value_specialization::built_in_registrations());
    registrations
}

#[cfg(test)]
pub(crate) use case_membership_specialization::CaseMembershipSpecializationRule;
#[cfg(test)]
pub(crate) use field_value_specialization::FieldValueSpecializationRule;
#[cfg(test)]
pub(crate) use sparse_conditional_constant_propagation::{
    IntegerRangeComparisonKind, IntegerRangePairComparisonKind,
};
#[cfg(test)]
pub(crate) use state_specialization::StateArgumentSpecializationRule;
#[cfg(test)]
use support::node_elision_accounting;

#[cfg(test)]
pub(crate) mod tests;

use optimization::PsiOptimizationSelections;
use optimization_core::OptimizationSelections;

use crate::{OrderedRuleRegistry, RuleRegistryError};

pub use catalog::{
    ORDERED_PSI_PASSES, PSI_PASS_CATALOG, PsiPassCatalogEntry, PsiPassTargetApplicability,
};
pub use control_flow_cleanup::{
    AdjacentBlockMergeRule, ConstantConditionalFoldRule, LinearEmptyBlockThreadRule,
    NonAdjacentBlockMergeRule, SharedJumpFusionRule,
};
#[cfg(test)]
pub(crate) use control_flow_cleanup::{
    PathQualifiedEmptyBlockThreadRule, UnreachablePrivateMachinePruneRule,
};
pub use copy_propagation::RedundantBlockParameterRule;
pub use dead_scalar_elimination::{
    DeadScalarLiteralEliminationRule, DeadUnconditionallyTotalScalarEliminationRule,
};
#[cfg(test)]
pub(crate) use global_value_numbering::{
    BitwiseAbsorbingLiteralIdentityRule, BitwiseNeutralLiteralIdentityRule,
    SaturatingMultiplyZeroAnnihilationRule, SaturatingNeutralArithmeticIdentityRule,
    WrappingMultiplyZeroAnnihilationRule, WrappingNeutralArithmeticIdentityRule,
    WrappingShiftZeroCountIdentityRule,
};
pub use global_value_numbering::{
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule, DominatorProofCertifiedScalarGvnRule,
    DominatorTotalScalarGvnRule, PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule, SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
    SameBlockProofCertifiedScalarCseRule, SameBlockTotalScalarCseRule,
};
pub use proof_check_elision::{
    LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule,
    LiveProofCertifiedExactIntegerSelfSubtractEliminationRule,
    LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule,
    LiveProofCertifiedExactSignedIntegerNegativeOneShiftRightEliminationRule,
    LiveProofCertifiedIntegerDivideByOneEliminationRule,
    LiveProofCertifiedIntegerIdentityEliminationRule,
    LiveProofCertifiedIntegerRemainderByOneEliminationRule,
    LiveProofCertifiedIntegerSelfDivideEliminationRule,
    LiveProofCertifiedIntegerSelfRemainderEliminationRule,
    LiveProofCertifiedIntegerZeroDividendEliminationRule,
    LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule,
    ProofCertifiedDeadScalarEliminationRule,
};
pub use sparse_conditional_constant_propagation::{
    BooleanEqualConstantsRule, BooleanNotConstantsRule, ExactIntegerAddConstantsRule,
    ExactIntegerCastConstantsRule, ExactIntegerDivideConstantsRule,
    ExactIntegerMultiplyConstantsRule, ExactIntegerRemainderConstantsRule,
    ExactIntegerShiftLeftConstantsRule, ExactIntegerShiftRightConstantsRule,
    ExactIntegerSubtractConstantsRule, IntegerBitwiseAndConstantsRule,
    IntegerBitwiseNotConstantsRule, IntegerBitwiseOrConstantsRule, IntegerBitwiseXorConstantsRule,
    IntegerEqualConstantRangeRule, IntegerEqualConstantsRule, IntegerEqualRangeConstantRule,
    IntegerEqualRangeRangeRule, IntegerLessOrEqualConstantRangeRule,
    IntegerLessOrEqualConstantsRule, IntegerLessOrEqualRangeConstantRule,
    IntegerLessOrEqualRangeRangeRule, IntegerLessThanConstantRangeRule,
    IntegerLessThanConstantsRule, IntegerLessThanRangeConstantRule, IntegerLessThanRangeRangeRule,
    IntegerWidenConstantsRule, SaturatingIntegerAddConstantsRule,
    SaturatingIntegerDivideConstantsRule, SaturatingIntegerMultiplyConstantsRule,
    SaturatingIntegerRemainderConstantsRule, SaturatingIntegerSubtractConstantsRule,
    WrappingIntegerAddConstantsRule, WrappingIntegerDivideConstantsRule,
    WrappingIntegerMultiplyConstantsRule, WrappingIntegerRemainderConstantsRule,
    WrappingIntegerShiftLeftConstantsRule, WrappingIntegerShiftRightConstantsRule,
    WrappingIntegerSubtractConstantsRule,
};

pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let projection = selections.project_psi();
    built_in_psi_registry_for_selections(projection.selections())
}

/// Construct at most one Psi pass registry from Psi's own target-neutral
/// selection vocabulary. The unified-build entrance above is a migration
/// adapter and performs only the exhaustive structural projection.
pub(crate) fn built_in_psi_registry_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let mut registries = built_in_psi_registries_for_selections(selections)?;
    if registries.len() > 1 {
        return Err(RuleRegistryError::UnsupportedOptimizationCombination);
    }
    Ok(registries
        .pop()
        .unwrap_or_else(|| OrderedRuleRegistry::new(Vec::new()).expect("empty registry is valid")))
}

/// Resolve exact selections in canonical catalog order.
pub(crate) fn built_in_psi_registries(
    selections: &OptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    let projection = selections.project_psi();
    built_in_psi_registries_for_selections(projection.selections())
}

/// Resolve exact Psi-owned selections in canonical catalog order.
pub(crate) fn built_in_psi_registries_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    if let Some(unsupported) = selections.as_slice().iter().find(|optimization| {
        !PSI_PASS_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **optimization)
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }

    PSI_PASS_CATALOG
        .iter()
        .copied()
        .filter(|entry| selections.contains(entry.optimization()))
        .map(|entry| catalog::registry_for_optimization(entry.optimization()))
        .collect()
}
