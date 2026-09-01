//! Optimizer module role: executable entrance. Independent liveness replay, exact comparison, and receipt admission.

mod constraints;
mod function_contract;
mod receipt;
mod replay;
mod shared;
mod structural;

#[cfg(test)]
mod tests;

use function_contract::validate_function;
use replay::replay_function;
use shared::*;
use structural::validate_structural_unit_roster;

pub fn validate_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
    plan: LivenessPlan,
) -> Result<ValidatedLiveness, LivenessError> {
    if !selected
        .selected_plan()
        .projected_structural_call_returns
        .is_empty()
    {
        return Err(LivenessError::ProjectedStructuralCallReturnUnsupported);
    }
    if plan.selected != selected.selected_identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
        || plan.structural_unit_functions.len()
            != selected.selected_plan().structural_unit_functions.len()
    {
        return Err(LivenessError::RootMismatch);
    }
    for (function_index, (selected_function, actual)) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&plan.functions)
        .enumerate()
    {
        let expected = replay_function(function_index, selected_function)?;
        validate_function(function_index, actual, &expected)?;
    }
    validate_structural_unit_roster(
        &selected.selected_plan().functions,
        &selected.selected_plan().structural_unit_functions,
        &plan.structural_unit_functions,
    )?;
    Ok(receipt::admit_validated_liveness(selected, plan))
}
