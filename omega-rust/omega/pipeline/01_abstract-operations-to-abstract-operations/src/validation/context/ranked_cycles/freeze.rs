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
    let mut expected =
        optimization_unit::reconstruct_psi_optimization_unit_seed(input.plan(), unit.fuel_schedule)
            .map_err(|_| {
                OptimizationUnitValidationError::VerifiedOptimizationUnitProjectionMismatch
            })?;
    // The crash-custody call lane re-derives each relocated call's
    // continuations from the callee's verifier-owned contract and the
    // caller's published crash ceiling: attach the module's verified
    // metadata to the reconstructed seed so both derive from the input's
    // authoritative context rather than trusting the transformed unit's
    // spelling (the transformed unit's own contract custody is checked
    // separately by the seed-projection validator).
    super::super::immutable_custody::attach_verified_structural_context(
        &mut expected,
        input.context().module(),
    )?;
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
        // The whole seed unit goes in: the scalar-call leg of relocation
        // replay needs the seed's transitive per-function effect table, which
        // is a unit-level product, while every other family needs only the
        // one function's seed spelling.
        let current_function = unit
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .ok_or(OptimizationUnitValidationError::RankedCycleFunctionMissing(
                machine,
            ))?;
        relocated_scalars::validate(machine, &expected, current_function, &machine_components)?;
    }
    Ok(())
}
