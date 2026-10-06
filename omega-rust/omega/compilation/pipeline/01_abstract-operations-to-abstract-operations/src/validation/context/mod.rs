//! Optimizer module role: executable entrance. Verified and transformed optimizer-context validation coordination.
//!
//! Both public routes reconstruct the admitted exact ranked components of a
//! unit this compile produced. They do not re-validate the unit, its context
//! indexes, seed projection or surviving frontiers.

use optimization_unit::PsiOptimizationUnit;
use optimization_unit_semantics::OptimizationUnitValidationError;

mod context_projection;
mod frontier_validation;
mod immutable_custody;
mod ranked_cycles;
mod seed_projection;

/// Validate a verified optimizer seed and retain its independently derived
/// cyclic-component analysis authority.
pub fn validate_verified_psi_cycle_components(
    verified: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    derive_cycle_components(verified.input(), verified.unit())
}

/// Validate a committed revision and retain its current canonical component
/// entries and exits under the immutable Terminal component identity.
pub fn validate_transformed_psi_cycle_components(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    derive_cycle_components(input, unit)
}

fn derive_cycle_components(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    // The unit is this compile's own seed or committed revision: only the
    // ranked cycle components are derived here, nothing is re-validated.
    let cycle_admission = ranked_cycles::validate_exact_ranked_cycles(input, unit)?;
    Ok(ranked_cycles::ValidatedOptimizerCycleComponents::new(
        cycle_admission.snapshot,
        ranked_cycles::ValidatedOptimizerRankingCertificates::new(cycle_admission.rankings),
    ))
}

pub use ranked_cycles::{ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates};
