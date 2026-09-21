//! Optimizer module role: executable entrance. Independent logical-spill reconstruction and admission.

mod receipt;
mod replay;
mod work;

use crate::{
    LogicalSpillOperationError, LogicalSpillOperationPlan, LogicalSpillOperationPolicy,
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLogicalSpillOperations,
    ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

pub fn validate_logical_spill_operations<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    choices: &ValidatedSpillChoices,
    plan: LogicalSpillOperationPlan,
) -> Result<ValidatedLogicalSpillOperations, LogicalSpillOperationError> {
    if plan.selected != selected.selected_identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.spill_choices != choices.receipt().identity()
        || plan.register_environment != choices.receipt().register_environment()
        || plan.allocator_availability != choices.receipt().allocator_availability()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
        || ranges.receipt().selected() != selected.selected_identity()
        || ranges.receipt().optimization_unit() != selected.optimization_unit_identity()
        || ranges.receipt().fuel_schedule() != selected.fuel_schedule_identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || choices.receipt().ranges() != ranges.receipt().identity()
        || choices.receipt().legality() != legality.receipt().identity()
        || choices.receipt().register_environment() != legality.receipt().register_environment()
        || choices.receipt().allocator_availability() != legality.receipt().allocator_availability()
        || plan.functions.len() != selected.selected_plan().functions.len()
        || plan.functions.len() != ranges.plan().functions.len()
        || plan.functions.len() != legality.plan().functions.len()
        || plan.functions.len() != choices.plan().functions.len()
    {
        return Err(LogicalSpillOperationError::RootMismatch);
    }
    if plan.policy
        != LogicalSpillOperationPolicy::SelectedActiveResidentInstructionResultU64StoreBeforePressureReloadBeforeFirstFutureFlexibleUseV1
    {
        return Err(LogicalSpillOperationError::UnsupportedPolicy);
    }
    for (function, plan_function) in plan.functions.iter().enumerate() {
        let expected = replay::replay_action(
            function,
            &selected.selected_plan().functions[function],
            &ranges.plan().functions[function],
            &legality.plan().functions[function],
            &choices.plan().functions[function],
        )?;
        if plan_function.machine != choices.plan().functions[function].machine {
            return Err(LogicalSpillOperationError::FunctionMismatch { function });
        }
        if plan_function.action.as_ref().is_some_and(|action| {
            action.storage.id.0 != 0
                || action.store.storage != action.storage.id
                || action.reload.storage != action.storage.id
                || action.reload.result.0 != 0
                || action
                    .rewrites
                    .iter()
                    .any(|rewrite| rewrite.result != action.reload.result)
        }) {
            return Err(LogicalSpillOperationError::NonCanonicalStorageIds { function });
        }
        if plan_function.action != expected {
            return Err(LogicalSpillOperationError::DecisionMismatch { function });
        }
    }
    let expected_usage = work::usage(&plan.functions)?;
    if plan.usage != expected_usage {
        return Err(LogicalSpillOperationError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(LogicalSpillOperationError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let receipt = receipt::receipt(&plan);
    Ok(ValidatedLogicalSpillOperations { plan, receipt })
}
