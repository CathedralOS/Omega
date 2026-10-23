//! Optimizer module role: executable entrance. Independent proof-check-elision validation routing.
//!
//! The adjacent catalog maps every exact producer identity to one validation
//! protocol: the dead-scalar elision, the operand substitution identity, or
//! one of the five integer constant laws, whose shared acceptance protocol
//! lives in `integer_constant_law`. Protocol leaves reconstruct semantics and
//! publish the exact validator identity; no sibling pass entrance recognizes
//! these rules.
use crate::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, validate_dead_scalar_node_candidate,
};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod candidate_validation;
mod identity_classification;
mod integer_constant_law;
mod rule_catalog;
mod same_operand;
mod unit_divisor;

pub(crate) use candidate_validation::validate_proof_certified_scalar_identity_candidate;

use integer_constant_law::validate_integer_constant_law_candidate;
use rule_catalog::ProofCheckValidationRoute;
pub(crate) use rule_catalog::is_proof_check_elision_rule;

/// Route one exact proof-check candidate to its independent semantic replay.
pub(super) fn validate_proof_check_elision_candidate(
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
        Some(ProofCheckValidationRoute::IntegerConstantLaw(law)) => {
            validate_integer_constant_law_candidate(input, candidate, &law)
        }
        None => Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch),
    }
}
