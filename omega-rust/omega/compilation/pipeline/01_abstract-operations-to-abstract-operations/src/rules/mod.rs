//! Optimizer module role: executable entrance. Built-in Psi optimization stage entrance.
//!
//! [`PSI_PASS_CATALOG`] is the only enable/disable and pass-order table. This
//! entrance filters exact selections through it and returns one ordered
//! registry per selected pass. Each pass is one file beside this one --
//! `<pass>.rs` holds that pass's visible local rule order and the contracts
//! its rules share -- and its rules are the files under `<pass>/`, one per
//! rule (or one table per rule family over a single proposal). `node_elision`
//! owns shared removal proposals and provenance/fuel accounting;
//! `accepted_obligations` rejoins the proof facts those proposals consume.
//! Constant, dominance, use-definition and cycle queries belong to `analyses`.
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

mod accepted_obligations;
mod catalog;
mod node_elision;
pub(crate) mod registry;

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
use node_elision::node_elision_accounting;
#[cfg(test)]
pub(crate) use sparse_conditional_constant_propagation::{
    IntegerRangeComparisonKind, IntegerRangePairComparisonKind,
};
#[cfg(test)]
pub(crate) use state_specialization::StateArgumentSpecializationRule;

#[cfg(test)]
pub(crate) mod tests;

use optimization::PsiOptimizationSelections;
#[cfg(any(test, feature = "test-support"))]
use optimization_core::OptimizationSelections;
use optimization_core::{
    OptimizationPassIdentity, OptimizationRuleContract, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity,
};

use crate::{OrderedRuleRegistry, RuleRegistryError};

#[cfg(any(test, feature = "test-support"))]
pub use catalog::PSI_PASS_CATALOG;

/// One scheduled Psi pass: either a fixed-point rewrite registry over
/// `PsiRewriteCandidate`s under analysis-manager custody, or a session-level
/// ranked-rewrite boundary whose candidates relocate authenticated cyclic
/// components through `VerifiedPsiOptimizationSession::from_transformed`.
/// Both steps publish the same decision, commit, manifest, and policy rows —
/// only the candidate declaration shape differs.
pub(crate) enum PsiPassStep {
    /// An ordered catalog rule registry iterating to a unit-local fixed point.
    RewriteRegistry(OrderedRuleRegistry),
    /// The unique-preheader loop-invariant scalar relocation boundary.
    LoopInvariantScalarMotion,
}

impl PsiPassStep {
    /// The named pass group this step publishes in the run manifest. An
    /// empty rewrite registry carries no pass row.
    pub(crate) fn pass(&self) -> Option<OptimizationPassIdentity> {
        match self {
            Self::RewriteRegistry(registry) => registry.pass(),
            Self::LoopInvariantScalarMotion => Some(
                OptimizationPassIdentity::from_canonical_bytes(
                    crate::ranked_rewrites::loop_invariant_scalar_motion::LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME,
                ),
            ),
        }
    }

    /// The ordered contract roster this step admits: the registry's rules, or
    /// the relocation boundary's single contract.
    pub(crate) fn contracts(&self) -> Vec<OptimizationRuleContract> {
        match self {
            Self::RewriteRegistry(registry) => registry.contracts().collect(),
            Self::LoopInvariantScalarMotion => {
                vec![crate::ranked_rewrites::loop_invariant_scalar_motion::contract()]
            }
        }
    }

    /// The step's ordered rule identities in manifest order.
    pub(crate) fn ordered_rules(&self) -> Vec<OptimizationRuleIdentity> {
        self.contracts()
            .iter()
            .map(|contract| contract.identity())
            .collect()
    }

    /// Canonical identity over the step's ordered rule identities.
    pub(crate) fn rule_set_identity(&self) -> OptimizationRuleSetIdentity {
        match self {
            Self::RewriteRegistry(registry) => registry.identity(),
            Self::LoopInvariantScalarMotion => OptimizationRuleSetIdentity::from_ordered_rules(&[
                crate::ranked_rewrites::loop_invariant_scalar_motion::rule_identity(),
            ])
            .expect("a single-rule schedule cannot duplicate"),
        }
    }
}
#[cfg(test)]
pub(crate) use control_flow_cleanup::{
    AdjacentBlockMergeRule, ConstantConditionalFoldRule, LinearEmptyBlockThreadRule,
    NonAdjacentBlockMergeRule, SharedJumpFusionRule,
};
#[cfg(test)]
pub(crate) use control_flow_cleanup::{
    PathQualifiedEmptyBlockThreadRule, UnreachablePrivateMachinePruneRule,
};
#[cfg(test)]
pub(crate) use copy_propagation::RedundantBlockParameterRule;
#[cfg(test)]
pub(crate) use dead_scalar_elimination::{
    DeadScalarLiteralEliminationRule, DeadUnconditionallyTotalScalarEliminationRule,
};
#[cfg(test)]
pub(crate) use global_value_numbering::{
    BitwiseAbsorbingLiteralIdentityRule, BitwiseNeutralLiteralIdentityRule,
    SaturatingMultiplyZeroAnnihilationRule, SaturatingNeutralArithmeticIdentityRule,
    WrappingMultiplyZeroAnnihilationRule, WrappingNeutralArithmeticIdentityRule,
    WrappingShiftZeroCountIdentityRule,
};
#[cfg(test)]
pub(crate) use global_value_numbering::{
    DominatorProofCertifiedCompatiblePolicyScalarGvnRule, DominatorProofCertifiedScalarGvnRule,
    DominatorTotalScalarGvnRule, PhiTranslatedObligationFreeScalarGvnRule,
    PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule,
    PhiTranslatedProofCertifiedScalarGvnRule, SameBlockProofCertifiedCompatiblePolicyScalarCseRule,
    SameBlockProofCertifiedScalarCseRule, SameBlockTotalScalarCseRule,
};
#[cfg(test)]
pub(crate) use proof_check_elision::{
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
#[cfg(any(test, feature = "test-support"))]
pub use sparse_conditional_constant_propagation::WrappingIntegerAddConstantsRule;
#[cfg(test)]
pub(crate) use sparse_conditional_constant_propagation::{
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
    WrappingIntegerDivideConstantsRule, WrappingIntegerMultiplyConstantsRule,
    WrappingIntegerRemainderConstantsRule, WrappingIntegerShiftLeftConstantsRule,
    WrappingIntegerShiftRightConstantsRule, WrappingIntegerSubtractConstantsRule,
};

#[cfg(any(test, feature = "test-support"))]
pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let projection = selections.project_psi();
    built_in_psi_registry_for_selections(projection.selections())
}

/// Construct at most one Psi pass registry from Psi's own target-neutral
/// selection vocabulary. The unified-build entrance above is a migration
/// adapter and performs only the exhaustive structural projection.
#[cfg(any(test, feature = "test-support"))]
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

/// Resolve exact selections in canonical catalog order. Registry-only shape:
/// session-level steps are not expressible here, so the relocation selection
/// rejects; the scheduled-step resolver is `built_in_psi_steps_for_selections`.
#[cfg(test)]
pub(crate) fn built_in_psi_registries(
    selections: &OptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    let projection = selections.project_psi();
    built_in_psi_registries_for_selections(projection.selections())
}

/// Resolve exact Psi-owned selections in canonical catalog order.
#[cfg(any(test, feature = "test-support"))]
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

/// Resolve exact Psi-owned selections to the ordered pass schedule: catalog
/// rewrite registries plus session-level ranked-rewrite steps.
pub(crate) fn built_in_psi_steps_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<Vec<PsiPassStep>, RuleRegistryError> {
    if let Some(unsupported) = selections.as_slice().iter().find(|optimization| {
        !catalog::PSI_PASS_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **optimization)
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    catalog::PSI_PASS_CATALOG
        .iter()
        .copied()
        .filter(|entry| selections.contains(entry.optimization()))
        .map(catalog::PsiPassCatalogEntry::step)
        .collect()
}
