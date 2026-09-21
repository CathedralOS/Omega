use std::collections::BTreeSet;

use crate::{
    StackSlotColoringError, StackSlotColoringPlan, StackSlotColoringValidationReceipt,
    stack_slot_coloring_identity,
};

pub(super) fn receipt(
    plan: &StackSlotColoringPlan,
) -> Result<StackSlotColoringValidationReceipt, StackSlotColoringError> {
    let assignment_count = plan.functions.iter().try_fold(0_usize, |total, function| {
        total
            .checked_add(function.assignments.len())
            .ok_or(StackSlotColoringError::WorkOverflow)
    })?;
    let distinct_slot_count = plan.functions.iter().try_fold(0_usize, |total, function| {
        let offsets = function
            .assignments
            .iter()
            .map(|assignment| assignment.spill_area_offset)
            .collect::<BTreeSet<_>>();
        total
            .checked_add(offsets.len())
            .ok_or(StackSlotColoringError::WorkOverflow)
    })?;
    let reused_assignment_count = assignment_count
        .checked_sub(distinct_slot_count)
        .ok_or(StackSlotColoringError::WorkOverflow)?;
    let max_function_spill_area_bytes = plan
        .functions
        .iter()
        .map(|function| function.spill_area_bytes)
        .max()
        .unwrap_or(0);
    Ok(StackSlotColoringValidationReceipt {
        identity: stack_slot_coloring_identity(plan),
        logical_spill_operations: plan.logical_spill_operations,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        policy: plan.policy,
        budget: plan.budget,
        usage: plan.usage,
        function_count: plan.functions.len(),
        assignment_count,
        distinct_slot_count,
        reused_assignment_count,
        max_function_spill_area_bytes,
    })
}
