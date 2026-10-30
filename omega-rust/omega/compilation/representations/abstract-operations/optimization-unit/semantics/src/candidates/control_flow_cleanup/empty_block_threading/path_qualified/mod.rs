//! Optimizer module role: executable entrance. Path-qualified empty-block validation.
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite};
use optimization_unit::{PsiOptimizationUnit, PsiRewriteCandidate};

mod replay;

/// Independently replay an all-predecessor empty-block bypass. Every incoming
/// edge remains its own output occurrence; the removed outgoing occurrence is
/// copied only onto that mutually exclusive edge antichain.
pub(crate) fn validate_path_qualified_empty_block_candidate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    super::super::super::contract::admit(
        input,
        candidate,
        &super::super::super::contract::PATH_QUALIFIED_EMPTY_BLOCK_THREAD,
    )?;
    if !candidate.substitutions().is_empty() {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    replay::validate(input, candidate)
}
