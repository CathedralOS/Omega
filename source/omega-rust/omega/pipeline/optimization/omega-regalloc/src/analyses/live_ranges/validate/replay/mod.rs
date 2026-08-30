//! Optimizer module role: executable entrance. Independent live-range reconstruction and admission join.

mod architectural_units;
mod canonical;
mod comparison;
mod constraints;
mod fragments;
mod function;

use std::collections::BTreeSet;

use crate::{LiveRangeError, LiveRangePlan, ValidatedLiveRanges, ValidatedLiveness};

pub(super) fn replay_live_ranges(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
    plan: LiveRangePlan,
) -> Result<ValidatedLiveRanges, LiveRangeError> {
    if plan.selected != selected.selected_identity()
        || plan.liveness != liveness.receipt().identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || plan.target != selected.selected_plan().target
        || plan.functions.len() != selected.selected_plan().functions.len()
        || plan.structural_unit_functions.len()
            != selected.selected_plan().structural_unit_functions.len()
        || plan.structural_unit_functions.len() != liveness.plan().structural_unit_functions.len()
    {
        return Err(LiveRangeError::RootMismatch);
    }
    let mut machines = BTreeSet::new();
    for (function_index, function) in plan
        .functions
        .iter()
        .chain(&plan.structural_unit_functions)
        .enumerate()
    {
        if !machines.insert(function.machine) {
            return Err(LiveRangeError::FunctionMismatch {
                function: function_index,
            });
        }
    }
    for (function_index, ((selected_function, live_function), actual)) in selected
        .selected_plan()
        .structural_unit_functions
        .iter()
        .zip(&liveness.plan().structural_unit_functions)
        .zip(&plan.structural_unit_functions)
        .enumerate()
    {
        let expected = function::replay_structural_function(
            function_index,
            selected_function.machine,
            live_function,
        )?;
        canonical::validate(function_index, actual)?;
        comparison::require_structural_function(function_index, actual, &expected)?;
    }
    for (function_index, ((selected_function, live_function), actual)) in selected
        .selected_plan()
        .functions
        .iter()
        .zip(&liveness.plan().functions)
        .zip(&plan.functions)
        .enumerate()
    {
        let expected = function::replay_function(function_index, selected_function, live_function)?;
        canonical::validate(function_index, actual)?;
        comparison::require_function(function_index, actual, &expected)?;
    }

    let receipt = super::receipt::build_receipt(&plan);
    Ok(ValidatedLiveRanges { plan, receipt })
}

#[cfg(test)]
mod tests;
