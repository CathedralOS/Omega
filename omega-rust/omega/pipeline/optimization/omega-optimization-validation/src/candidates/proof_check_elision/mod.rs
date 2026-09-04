//! Optimizer module role: executable entrance. Independent proof-check-elision validation routing.
//!
//! The adjacent catalog maps every exact producer identity to one validation
//! protocol. Protocol leaves reconstruct semantics and publish the exact
//! validator identity; no sibling pass entrance recognizes these rules.

use super::*;

mod candidate_validation;
mod identity_classification;
mod rule_catalog;
mod same_operand;
mod unit_divisor;

pub use candidate_validation::validate_proof_certified_scalar_identity_candidate;
pub use same_operand::{
    validate_proof_certified_exact_integer_self_subtract_candidate,
    validate_proof_certified_integer_self_divide_candidate,
    validate_proof_certified_integer_self_remainder_candidate,
};
pub use unit_divisor::{
    validate_proof_certified_integer_remainder_by_one_candidate,
    validate_proof_certified_signed_integer_remainder_by_negative_one_candidate,
};

use rule_catalog::ProofCheckValidationRoute;
pub(crate) use rule_catalog::is_proof_check_elision_rule;

/// Route one exact proof-check candidate to its independent semantic replay.
pub fn validate_proof_check_elision_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match ProofCheckValidationRoute::for_rule(candidate.rule()) {
        Some(ProofCheckValidationRoute::DeadScalar) => {
            validate_dead_scalar_node_candidate(input, candidate)
        }
        Some(ProofCheckValidationRoute::OperandSubstitution) => {
            validate_proof_certified_scalar_identity_candidate(input, candidate)
        }
        Some(ProofCheckValidationRoute::SelfSubtract) => {
            validate_proof_certified_exact_integer_self_subtract_candidate(input, candidate)
        }
        Some(ProofCheckValidationRoute::SelfRemainder) => {
            validate_proof_certified_integer_self_remainder_candidate(input, candidate)
        }
        Some(ProofCheckValidationRoute::SelfDivide) => {
            validate_proof_certified_integer_self_divide_candidate(input, candidate)
        }
        Some(ProofCheckValidationRoute::RemainderByOne) => {
            validate_proof_certified_integer_remainder_by_one_candidate(input, candidate)
        }
        Some(ProofCheckValidationRoute::RemainderByNegativeOne) => {
            validate_proof_certified_signed_integer_remainder_by_negative_one_candidate(
                input, candidate,
            )
        }
        None => Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    }
}
