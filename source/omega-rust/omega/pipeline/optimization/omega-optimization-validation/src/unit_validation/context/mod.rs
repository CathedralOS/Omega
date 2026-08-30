//! Optimizer module role: executable entrance. Verified and transformed optimizer-context validation coordination.
//!
//! Both public routes validate the complete unit first, then replay immutable
//! context indexes, seed/fact projection, and surviving frontier custody. The
//! only policy difference is whether the initial revision identity is required.

use super::*;

mod context_projection;
mod frontier_validation;
mod immutable_custody;
mod seed_projection;

pub fn validate_verified_psi_optimization_unit(
    verified: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(verified.input(), verified.unit(), true)
}

/// Validate a committed optimization revision while retaining the immutable
/// verifier context that authorized its proof and ownership facts.
///
/// Unlike [`validate_verified_psi_optimization_unit`], this permits the unit's
/// revision identity and executable shape to differ from the initial verified
/// seed. The admitted-fact projection and every surviving provenance frontier
/// must still match the original artifact exactly.
pub fn validate_transformed_psi_optimization_unit(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(input, unit, false)
}

pub(crate) fn validate_psi_optimization_unit_with_context(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    require_initial_revision: bool,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit(unit)?;
    let projected_context = context_projection::validate_context_projection(input, unit)?;
    seed_projection::validate_seed_projection(
        input,
        unit,
        &projected_context,
        require_initial_revision,
    )?;
    frontier_validation::validate_surviving_frontiers(input, unit)
}
