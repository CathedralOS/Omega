//! Optimizer module role: executable entrance. Bounded canonical coloring proposal.

mod first_fit;
mod intervals;
mod work;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FunctionStackSlotColoring, StackSlotColoringError, StackSlotColoringPlan,
    StackSlotColoringPolicy, ValidatedLogicalSpillOperations,
};

pub(in crate::allocation::stack_slot_coloring) use first_fit::color_intervals_first_fit;
pub(in crate::allocation::stack_slot_coloring) use intervals::StackSlotInterval;
use intervals::intervals_for_function;

pub(super) fn compute_stack_slot_coloring(
    source: &ValidatedLogicalSpillOperations,
    policy: StackSlotColoringPolicy,
    budget: OptimizationWorkBudget,
) -> Result<StackSlotColoringPlan, StackSlotColoringError> {
    admit_source(source)?;
    admit_policy(policy)?;
    let functions = replay_functions(source)?;
    let usage = usage(&functions)?;
    if !usage.within(budget) {
        return Err(StackSlotColoringError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    let receipt = source.receipt();
    Ok(StackSlotColoringPlan {
        logical_spill_operations: receipt.identity(),
        register_environment: receipt.register_environment(),
        allocator_availability: receipt.allocator_availability(),
        optimization_unit: receipt.optimization_unit(),
        fuel_schedule: receipt.fuel_schedule(),
        policy,
        budget,
        usage,
        functions,
    })
}

pub(super) fn admit_source(
    source: &ValidatedLogicalSpillOperations,
) -> Result<(), StackSlotColoringError> {
    let plan = source.plan();
    let receipt = source.receipt();
    if receipt.identity() != crate::logical_spill_operation_identity(plan)
        || receipt.register_environment() != plan.register_environment
        || receipt.allocator_availability() != plan.allocator_availability
        || receipt.optimization_unit() != plan.optimization_unit
        || receipt.fuel_schedule() != plan.fuel_schedule
        || receipt.function_count() != plan.functions.len()
    {
        return Err(StackSlotColoringError::RootMismatch);
    }
    Ok(())
}

pub(super) fn admit_policy(policy: StackSlotColoringPolicy) -> Result<(), StackSlotColoringError> {
    if policy != StackSlotColoringPolicy::BlockLocalNonAddressUnsignedU64ClosedIntervalFirstFitV1 {
        return Err(StackSlotColoringError::UnsupportedPolicy);
    }
    Ok(())
}

pub(super) fn replay_functions(
    source: &ValidatedLogicalSpillOperations,
) -> Result<Vec<FunctionStackSlotColoring>, StackSlotColoringError> {
    source
        .plan()
        .functions
        .iter()
        .enumerate()
        .map(|(function, logical)| {
            let intervals = intervals_for_function(function, logical)?;
            color_intervals_first_fit(function, logical.machine, intervals)
        })
        .collect()
}

pub(super) fn usage(
    functions: &[FunctionStackSlotColoring],
) -> Result<OptimizationWorkUsage, StackSlotColoringError> {
    work::usage(functions)
}
