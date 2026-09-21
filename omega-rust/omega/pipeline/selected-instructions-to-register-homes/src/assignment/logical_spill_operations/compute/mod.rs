//! Optimizer module role: executable entrance. Bounded logical-spill proposal coordination.

pub(super) mod action;
mod work;

use optimization_core::OptimizationWorkBudget;

use crate::{
    FunctionLogicalSpillOperations, LogicalSpillOperationError, LogicalSpillOperationPlan,
    LogicalSpillOperationPolicy, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

pub(super) fn compute_terminal_logical_spill_operations<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    choices: &ValidatedSpillChoices,
    policy: LogicalSpillOperationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<LogicalSpillOperationPlan, LogicalSpillOperationError> {
    admit_roots(selected, ranges, legality, choices)?;
    if policy
        != LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1
    {
        return Err(LogicalSpillOperationError::UnsupportedPolicy);
    }
    let mut functions = Vec::with_capacity(choices.plan().functions.len());
    for function in 0..choices.plan().functions.len() {
        functions.push(FunctionLogicalSpillOperations {
            machine: choices.plan().functions[function].machine,
            action: action::compute_action(
                function,
                &selected.selected_plan().functions[function],
                &ranges.plan().functions[function],
                &legality.plan().functions[function],
                &choices.plan().functions[function],
            )?,
        });
    }
    let usage = work::usage(&functions)?;
    if !usage.within(budget) {
        return Err(LogicalSpillOperationError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(LogicalSpillOperationPlan {
        selected: selected.selected_identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        spill_choices: choices.receipt().identity(),
        register_environment: choices.receipt().register_environment(),
        allocator_availability: choices.receipt().allocator_availability(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
    })
}

fn admit_roots<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    choices: &ValidatedSpillChoices,
) -> Result<(), LogicalSpillOperationError> {
    if ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || choices.receipt().ranges() != ranges.receipt().identity()
        || choices.receipt().legality() != legality.receipt().identity()
        || choices.receipt().register_environment() != legality.receipt().register_environment()
        || choices.receipt().allocator_availability() != legality.receipt().allocator_availability()
        || selected.selected_plan().functions.len() != ranges.plan().functions.len()
        || selected.selected_plan().functions.len() != legality.plan().functions.len()
        || selected.selected_plan().functions.len() != choices.plan().functions.len()
    {
        return Err(LogicalSpillOperationError::RootMismatch);
    }
    Ok(())
}
