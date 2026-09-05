//! Recovery-classification proposal assembly over validated roots and bounded work.

mod function_classification;
mod immediate_eligibility;
mod work_usage;

#[cfg(test)]
mod tests;

use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    RecoveryClassificationError, RecoveryClassificationPlan, RecoveryClassificationPolicy,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedSelectedAnalysis,
    ValidatedSpillChoices,
};

pub(crate) fn compute_terminal_recovery_classifications<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    policy: RecoveryClassificationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<RecoveryClassificationPlan, RecoveryClassificationError> {
    validate_roots(selected, ranges, legality, spill_choices)?;
    if policy != RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 {
        return Err(RecoveryClassificationError::UnsupportedPolicy);
    }
    let usage = work_usage::required(selected, ranges, spill_choices)?;
    if !usage.within(budget) {
        return Err(RecoveryClassificationError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let functions = selected
        .selected_plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .zip(&legality.plan().functions)
        .zip(&spill_choices.plan().functions)
        .enumerate()
        .map(|(function, (((selected, ranges), legality), choices))| {
            function_classification::classify(function, selected, ranges, legality, choices)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RecoveryClassificationPlan {
        selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
        allocator_availability: legality.receipt().allocator_availability(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn validate_roots(
    selected: &impl ValidatedSelectedAnalysis,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
) -> Result<(), RecoveryClassificationError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().ranges() != ranges.receipt().identity()
        || spill_choices.receipt().legality() != legality.receipt().identity()
        || spill_choices.receipt().register_environment()
            != legality.receipt().register_environment()
        || selected.selected_plan().functions.len() != ranges.plan().functions.len()
        || selected.selected_plan().functions.len() != legality.plan().functions.len()
        || selected.selected_plan().functions.len() != spill_choices.plan().functions.len()
    {
        return Err(RecoveryClassificationError::RootMismatch);
    }
    Ok(())
}
