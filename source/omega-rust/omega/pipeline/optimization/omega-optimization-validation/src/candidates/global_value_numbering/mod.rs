//! Optimizer module role: executable entrance. Scalar common-subexpression validation coordination.
//!
//! The entrance validates candidate custody once, classifies the exact named
//! rule, and then selects one of two rewrite protocols:
//!
//! 1. same-block or dominating elimination;
//! 2. phi-translated join-parameter synthesis.
//!
//! Expression classification, dominance reconstruction, and proof admission
//! are independent evidence services used by those protocols.

use super::*;

mod admission;
mod dominance_reconstruction;
mod expression_keys;
mod local_and_dominating;
mod phi_translated;
mod rule_catalog;
mod total_scalar_identity;

pub use total_scalar_identity::validate_total_scalar_identity_candidate;

use rule_catalog::{
    ScalarCseProofClass, ScalarCseScope, phi_translated_proof_class, scoped_proof_class,
};

pub(crate) use admission::independently_accepted_operation_fact;
pub(crate) use dominance_reconstruction::{
    independent_reachable_dominators, independently_replacement_dominates_uses,
};

/// Independently validate and apply one same-block common-subexpression elimination.
pub fn validate_local_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_candidate_origin(input, candidate)?;
    let scope = ScalarCseScope::SameBlock;
    let proof_class = scoped_proof_class(scope, candidate.rule())?;
    local_and_dominating::validate_scalar_common_subexpression_candidate(
        input,
        candidate,
        scope,
        proof_class,
    )
}

/// Independently validate and apply one cross-block dominating
/// common-subexpression elimination.
pub fn validate_dominating_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_candidate_origin(input, candidate)?;
    let scope = ScalarCseScope::Dominating;
    let proof_class = scoped_proof_class(scope, candidate.rule())?;
    local_and_dominating::validate_scalar_common_subexpression_candidate(
        input,
        candidate,
        scope,
        proof_class,
    )
}

/// Independently validate an expression translated through every incoming
/// binding of an acyclic join and synthesize its result as a join parameter.
pub fn validate_phi_translated_scalar_common_subexpression_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    validate_candidate_origin(input, candidate)?;
    let proof_class = phi_translated_proof_class(candidate.rule())?;
    phi_translated::validate_phi_translated_scalar_common_subexpression_candidate(
        input,
        candidate,
        proof_class,
    )
}

fn validate_candidate_origin(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(input)?;
    if candidate.input() != input.identity {
        return Err(OptimizationUnitValidationError::CandidateInputMismatch);
    }
    Ok(())
}
