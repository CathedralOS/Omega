//! Optimizer module role: validation leaf. Immutable executable-block comparison for the admitted ranked component.

use super::*;

pub(super) fn validate_frozen_component_blocks(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
    components: &[(MachineId, BTreeSet<BlockId>)],
) -> Result<(), OptimizationUnitValidationError> {
    if components.is_empty() {
        return Ok(());
    }
    let expected = omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
        input.plan(),
        unit.fuel_schedule,
    )
    .map_err(|_| OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch)?;
    for (machine, members) in components {
        let expected_function = expected
            .functions
            .iter()
            .find(|function| function.machine == *machine)
            .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
                *machine,
            ))?;
        let current_function = unit
            .functions
            .iter()
            .find(|function| function.machine == *machine)
            .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
                *machine,
            ))?;
        for block in members {
            let expected_block = expected_function
                .blocks
                .iter()
                .find(|candidate| candidate.id == *block);
            let current_block = current_function
                .blocks
                .iter()
                .find(|candidate| candidate.id == *block);
            if expected_block.is_none() || expected_block != current_block {
                return Err(
                    OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
                        machine: *machine,
                        block: *block,
                    },
                );
            }
        }
    }
    Ok(())
}
