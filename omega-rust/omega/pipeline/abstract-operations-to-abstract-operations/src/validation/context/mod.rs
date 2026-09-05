//! Optimizer module role: executable entrance. Verified and transformed optimizer-context validation coordination.
//!
//! Both public routes first reconstruct any admitted exact ranked component,
//! then validate the complete unit and replay immutable context indexes,
//! seed/fact projection, and surviving frontier custody. The only policy
//! difference is whether the initial revision identity is required.

use super::*;

mod context_projection;
mod frontier_validation;
mod immutable_custody;
mod ranked_cycles;
mod seed_projection;

pub fn validate_verified_psi_optimization_unit(
    verified: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(verified.input(), verified.unit(), true).map(drop)
}

/// Validate a verified optimizer seed and retain its independently derived
/// cyclic-component analysis authority.
pub fn validate_verified_psi_cycle_components(
    verified: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
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
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(input, unit, false).map(drop)
}

/// Validate a committed revision and retain its current canonical component
/// entries and exits under the immutable Terminal component identity.
pub fn validate_transformed_psi_cycle_components(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    validate_psi_optimization_unit_with_context(input, unit, false)
}

/// Reauthenticate a replayable component snapshot against both Terminal and
/// the current optimizer graph.
pub fn validate_psi_cycle_component_snapshot(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    candidate: &optimization_unit::OptimizerCycleComponentSnapshot,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    let validated = validate_psi_optimization_unit_with_context(input, unit, false)?;
    if candidate != validated.snapshot() {
        return Err(OptimizationUnitValidationError::RankedCycleComponentSnapshotMismatch);
    }
    Ok(validated)
}

fn validate_psi_optimization_unit_with_context(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    require_initial_revision: bool,
) -> Result<ranked_cycles::ValidatedOptimizerCycleComponents, OptimizationUnitValidationError> {
    let cycle_admission = ranked_cycles::validate_exact_ranked_cycles(input, unit)?;
    validate_psi_optimization_unit_with_admitted_cycle_machines(unit, &cycle_admission.machines)?;
    let projected_context = context_projection::validate_context_projection(input, unit)?;
    seed_projection::validate_seed_projection(
        input,
        unit,
        &projected_context,
        require_initial_revision,
    )?;
    frontier_validation::validate_surviving_frontiers(input, unit)?;
    Ok(ranked_cycles::ValidatedOptimizerCycleComponents::new(
        cycle_admission.snapshot,
        ranked_cycles::ValidatedOptimizerRankingCertificates::new(cycle_admission.rankings),
    ))
}

#[cfg(test)]
mod tests;

pub use ranked_cycles::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_psi_ranking_certificate_snapshot,
};
