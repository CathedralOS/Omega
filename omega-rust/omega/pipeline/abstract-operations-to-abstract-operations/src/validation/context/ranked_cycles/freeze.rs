//! Optimizer module role: validation leaf. Frozen ranked-block preservation coordination.

use super::super::super::BTreeMap;
use super::{
    MachineId, OptimizationUnitValidationError, OptimizerCycleComponent, PsiOptimizationUnit,
};
mod relocated_scalars;

pub(super) fn validate_frozen_component_blocks(
    input: &terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    components: &[OptimizerCycleComponent],
) -> Result<(), OptimizationUnitValidationError> {
    if components.is_empty() {
        return Ok(());
    }
    let expected =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), unit.fuel_schedule)
            .map_err(|_| {
                OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
            })?;
    let mut machines = BTreeMap::<MachineId, Vec<&OptimizerCycleComponent>>::new();
    for component in components {
        machines
            .entry(component.id.machine)
            .or_default()
            .push(component);
    }
    // Prefix definitions and exit observations matter for unranked, Natural,
    // and countdown cycles alike. Topology equality alone cannot preserve
    // them; the relocation normalization retains every source-owned field
    // while admitting only scalar motion into a component's own
    // unique preheader. Immutable signature/contract and accepted-fact
    // custody are checked separately by the enclosing context validator. The
    // bare seed has not yet acquired that verified metadata.
    for (machine, machine_components) in machines {
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
        relocated_scalars::validate(
            machine,
            expected_function,
            current_function,
            &machine_components,
        )?;
    }
    Ok(())
}
