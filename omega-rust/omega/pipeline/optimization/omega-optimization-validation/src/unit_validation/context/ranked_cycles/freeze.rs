//! Optimizer module role: validation leaf. Frozen ranked-block preservation coordination.

use super::*;

mod normalized_component;

pub(super) fn validate_frozen_component_blocks(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    components: &[OptimizerCycleComponent],
    rankings: &[OptimizerUnsignedCountdownRankingCertificate],
) -> Result<(), OptimizationUnitValidationError> {
    if components.is_empty() {
        return Ok(());
    }
    let expected = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        unit.fuel_schedule,
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    for component in components {
        let machine = component.id.machine;
        let expected_function = expected
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
                machine,
            ))?;
        let current_function = unit
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
                machine,
            ))?;
        let certificates = rankings
            .iter()
            .filter(|certificate| certificate.component == component.id)
            .collect::<Vec<_>>();
        let [certificate] = certificates.as_slice() else {
            return Err(
                OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                    machine,
                    block: component
                        .members
                        .first()
                        .copied()
                        .unwrap_or(current_function.entry),
                },
            );
        };
        normalized_component::validate(
            expected_function,
            current_function,
            component,
            certificate,
        )?;
    }
    Ok(())
}
