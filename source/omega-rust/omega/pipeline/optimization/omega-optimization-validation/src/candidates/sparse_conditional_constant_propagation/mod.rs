//! Optimizer module role: executable entrance. Independent SCCP candidate validation entrance.
//!
//! This entrance owns the exact rule and patch routing join. Integer and
//! boolean acceptance, typed-range comparison, observation equality, exact
//! arithmetic, and SCCP snapshot reconstruction descend into named leaves.

use super::*;

mod boolean_candidate;
mod boolean_evaluation;
mod integer_candidate;
mod integer_evaluation;
mod observation;
mod snapshot_reconstruction;

pub use boolean_candidate::validate_boolean_evaluation_candidate;
pub use integer_candidate::validate_integer_evaluation_candidate;

#[cfg(test)]
pub(crate) use boolean_evaluation::{
    ValidatedIntegerRangeComparisonKind, ValidatedIntegerRangePairComparisonKind,
    independently_evaluate_integer_range_comparison,
    independently_evaluate_integer_range_pair_comparison,
    independently_validated_integer_range_comparison_kind,
    independently_validated_integer_range_pair_comparison_kind,
};
pub(crate) use integer_evaluation::literal_boolean_fact;
pub(crate) use observation::{observation_at, same_closed_scalar_observation};
pub(crate) use snapshot_reconstruction::{
    scalar_value_definition, validator_scalar_constant_facts,
};

pub fn validate_scalar_evaluation_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    match candidate.patch() {
        PsiRewritePatch::ReplaceIntegerOperationWithConstant(_) => {
            validate_integer_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::ReplaceBooleanOperationWithConstant(_) => {
            validate_boolean_evaluation_candidate(input, candidate)
        }
        PsiRewritePatch::RemoveRedundantBlockParameter(_)
        | PsiRewritePatch::FoldConstantConditional(_)
        | PsiRewritePatch::ThreadLinearEmptyBlock(_)
        | PsiRewritePatch::ThreadPathQualifiedEmptyBlock(_)
        | PsiRewritePatch::MergeAdjacentBlock(_)
        | PsiRewritePatch::MergeNonAdjacentBlock(_)
        | PsiRewritePatch::FuseSharedTerminalJump(_)
        | PsiRewritePatch::RemoveDeadScalarNode(_)
        | PsiRewritePatch::EliminateLocalScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminateDominatedScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminatePhiTranslatedScalarCommonSubexpression(_)
        | PsiRewritePatch::EliminateProofCertifiedScalarIdentity(_)
        | PsiRewritePatch::EliminateTotalScalarIdentity(_)
        | PsiRewritePatch::PruneUnreachablePrivateMachines(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
    }
}
