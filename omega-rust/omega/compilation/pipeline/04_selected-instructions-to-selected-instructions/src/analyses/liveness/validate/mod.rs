//! Optimizer module role: executable entrance. Independent liveness replay, exact comparison, and receipt admission.

mod constraints;
mod function_contract;
mod instruction_order;
mod receipt;
mod replay;

use crate::analyses::liveness::{LivenessError, ValidatedLiveness};
use function_contract::validate_function;
use replay::replay_function;
use selected_instructions::LivenessPlan;

pub fn validate_liveness(
    selected: &impl crate::ValidatedSelectedAnalysis,
    plan: LivenessPlan,
) -> Result<ValidatedLiveness, LivenessError> {
    validate_liveness_carrying(selected, plan, &[])
}

/// `validate_liveness`, except that a function marked in `carried` is not
/// replayed: its rows were copied unchanged from a validated predecessor plan
/// for an identical selected function, and replay reads only that function.
pub(crate) fn validate_liveness_carrying(
    selected: &impl crate::ValidatedSelectedAnalysis,
    plan: LivenessPlan,
    carried: &[bool],
) -> Result<ValidatedLiveness, LivenessError> {
    if plan.selected != selected.selected_identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
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
        if carried.get(function_index) == Some(&true) {
            continue;
        }
        let expected = replay_function(function_index, selected_function)?;
        validate_function(function_index, actual, &expected)?;
    }
    Ok(receipt::admit_validated_liveness(selected, plan))
}
