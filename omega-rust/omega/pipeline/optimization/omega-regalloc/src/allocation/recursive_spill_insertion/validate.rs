//! Independent replay comparison and receipt sealing.

use crate::{
    RecursiveSpillInsertionError, RecursiveSpillInsertionPlan, RecursiveSpillInsertionReceipt,
    ValidatedGeneralizedSpillInsertion, ValidatedGeneralizedSpillRecoveryActions,
    ValidatedRecursiveSpillInsertion, recursive_spill_insertion_identity,
};

pub fn validate_recursive_spill_insertion(
    base: &ValidatedGeneralizedSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
    plan: RecursiveSpillInsertionPlan,
) -> Result<ValidatedRecursiveSpillInsertion, RecursiveSpillInsertionError> {
    if plan.generalized_spill_insertion != base.receipt().identity()
        || plan.recovery_actions != recovery.receipt().identity()
        || plan.register_environment != recovery.plan().register_environment
        || plan.allocator_availability != recovery.plan().allocator_availability
        || plan.optimization_unit != recovery.receipt().optimization_unit()
        || plan.fuel_schedule != recovery.receipt().fuel_schedule()
        || plan.functions.len() != base.plan().functions.len()
    {
        return Err(RecursiveSpillInsertionError::RootMismatch);
    }
    let expected = super::replay::replay(base, recovery, plan.policy, plan.budget)?;
    for (function, (candidate, replayed)) in
        plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if candidate.machine != replayed.machine {
            return Err(RecursiveSpillInsertionError::FunctionMismatch { function });
        }
        if candidate.spill_area_bytes != replayed.spill_area_bytes
            || candidate.slots != replayed.slots
        {
            return Err(RecursiveSpillInsertionError::NonCanonicalSlots { function });
        }
        if candidate.schedule != replayed.schedule {
            return Err(RecursiveSpillInsertionError::NonCanonicalSchedule { function });
        }
    }
    if plan.usage != expected.usage {
        return Err(RecursiveSpillInsertionError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(RecursiveSpillInsertionError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let action_count = plan.functions.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.slots.len())
            .ok_or(RecursiveSpillInsertionError::WorkOverflow)
    })?;
    let event_count = plan.functions.iter().try_fold(0_usize, |total, row| {
        total
            .checked_add(row.schedule.len())
            .ok_or(RecursiveSpillInsertionError::WorkOverflow)
    })?;
    let receipt = RecursiveSpillInsertionReceipt {
        identity: recursive_spill_insertion_identity(&plan),
        generalized_spill_insertion: plan.generalized_spill_insertion,
        recovery_actions: plan.recovery_actions,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        function_count: plan.functions.len(),
        action_count,
        event_count,
        max_spill_area_bytes: plan
            .functions
            .iter()
            .map(|row| row.spill_area_bytes)
            .max()
            .unwrap_or(0),
    };
    Ok(ValidatedRecursiveSpillInsertion { plan, receipt })
}
