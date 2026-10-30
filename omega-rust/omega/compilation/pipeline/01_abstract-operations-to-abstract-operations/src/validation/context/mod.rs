//! Optimizer module role: executable entrance. Verified and transformed optimizer-context validation coordination.
//!
//! Both session routes first retain exact runtime-evidence declaration custody
//! against their immutable verified Terminal input, then reconstruct admitted
//! ranked components. This does not repeat body, seed-projection, frontier or
//! snapshot-history validation; transformed bodies keep those own readers.

use optimization_unit::PsiOptimizationUnit;
use optimization_unit_semantics::OptimizationUnitValidationError;

mod ranked_cycles;

/// Validate a verified optimizer seed and retain its independently derived
/// cyclic-component analysis authority.
pub(crate) fn validate_verified_psi_context(
    verified: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    derive_context(verified.input(), verified.unit())
}

/// Validate a committed revision and retain its current canonical component
/// entries and exits under the immutable Terminal component identity.
pub(crate) fn validate_transformed_psi_context(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    derive_context(input, unit)
}

fn derive_context(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    // This authorization boundary receives the immutable verified input even
    // for an identity transform. Rehashing a changed unit cannot authorize
    // removing its evidence catalog or declaration lanes. Current executable
    // snapshot correspondence remains the independent unit reader's task.
    optimization_unit::validate_runtime_evidence_context(
        unit,
        input.plan().psi,
        input.context().module(),
    )
    .map_err(|reason| OptimizationUnitValidationError::InvalidRuntimeEvidence { reason })?;
    let cycle_admission = ranked_cycles::validate_exact_ranked_cycles(input, unit)?;
    Ok(ranked_cycles::ValidatedOptimizerCycleComponents::new(
        cycle_admission.snapshot,
        ranked_cycles::ValidatedOptimizerRankingCertificates::new(cycle_admission.rankings),
    ))
}

pub(crate) use ranked_cycles::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
};
