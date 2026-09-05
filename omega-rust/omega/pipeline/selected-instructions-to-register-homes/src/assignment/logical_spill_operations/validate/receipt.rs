use crate::{
    LogicalSpillOperationPlan, LogicalSpillOperationValidationReceipt,
    logical_spill_operation_identity,
};

pub(super) fn receipt(plan: &LogicalSpillOperationPlan) -> LogicalSpillOperationValidationReceipt {
    let planned_function_count = plan
        .functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let rewritten_use_count = plan
        .functions
        .iter()
        .filter_map(|function| function.action.as_ref())
        .map(|action| action.rewrites.len())
        .sum();
    LogicalSpillOperationValidationReceipt {
        identity: logical_spill_operation_identity(plan),
        selected: plan.selected,
        ranges: plan.ranges,
        legality: plan.legality,
        spill_choices: plan.spill_choices,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.functions.len(),
        planned_function_count,
        store_count: planned_function_count,
        reload_count: planned_function_count,
        rewritten_use_count,
    }
}
