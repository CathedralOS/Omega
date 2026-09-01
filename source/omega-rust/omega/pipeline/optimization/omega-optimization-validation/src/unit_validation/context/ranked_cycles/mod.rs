//! Optimizer module role: stage group. Exact ranked-cycle context replay and immutable-component fencing.

use super::super::*;

mod freeze;
mod topology;

pub(super) fn validate_exact_ranked_cycles(
    input: &omega_psi_to_abstract_operations::VerifiedPsiOptimizationInput,
    unit: &PsiOptimizationUnit,
) -> Result<function_structure::ControlCyclePolicy, OptimizationUnitValidationError> {
    let components = topology::rederive_exact_components(input.context().module(), unit)?;
    freeze::validate_frozen_component_blocks(input, unit, &components)?;
    let mut policy = function_structure::ControlCyclePolicy::default();
    for (machine, _) in components {
        policy.admit(machine);
    }
    Ok(policy)
}
