//! Optimizer module role: executable entrance. Independent SCCP candidate validation entrance.
//!
//! This entrance owns the exact rule and patch routing join. Integer and
//! boolean acceptance, typed-range comparison, observation equality, exact
//! arithmetic, and SCCP snapshot reconstruction descend into named leaves.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate, PsiRewritePatch};

mod boolean_candidate;
mod boolean_evaluation;
mod integer_candidate;
mod integer_evaluation;
mod observation;
mod snapshot_reconstruction;

pub(crate) use boolean_candidate::validate_boolean_evaluation_candidate;
pub(crate) use integer_candidate::validate_integer_evaluation_candidate;

pub(crate) use integer_evaluation::literal_boolean_fact;
pub(crate) use observation::{observation_at, same_closed_scalar_observation};
pub(crate) use optimization_unit::scalar_value_definition;
pub(crate) use snapshot_reconstruction::validator_scalar_constant_facts;

pub(super) fn validate_scalar_evaluation_candidate(
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
        | PsiRewritePatch::PruneUnreachablePrivateMachines(_)
        | PsiRewritePatch::SpecializeStateArgument(_)
        | PsiRewritePatch::SpecializeCaseMembership(_)
        | PsiRewritePatch::SpecializeFieldValue(_) => {
            Err(OptimizationUnitValidationError::CandidatePatchMismatch)
        }
    }
}
