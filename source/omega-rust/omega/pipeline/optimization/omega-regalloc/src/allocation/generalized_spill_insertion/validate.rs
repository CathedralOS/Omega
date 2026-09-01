//! Independent replay comparison and receipt sealing.

use crate::{
    GeneralizedSpillInsertionError, GeneralizedSpillInsertionPlan,
    GeneralizedSpillInsertionReceipt, ValidatedAbstractSpillInsertion,
    ValidatedGeneralizedSpillInsertion, ValidatedSpillRecoveryActions,
    generalized_spill_insertion_identity,
};

pub fn validate_generalized_spill_insertion(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    plan: GeneralizedSpillInsertionPlan,
) -> Result<ValidatedGeneralizedSpillInsertion, GeneralizedSpillInsertionError> {
    validate_roots(first, second, &plan)?;
    let expected = super::replay::replay(first, second, plan.policy, plan.budget)?;
    for (function, (candidate, replayed)) in
        plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if candidate.machine != replayed.machine {
            return Err(GeneralizedSpillInsertionError::FunctionMismatch { function });
        }
        if candidate.spill_area_bytes != replayed.spill_area_bytes
            || candidate.slots != replayed.slots
        {
            return Err(GeneralizedSpillInsertionError::NonCanonicalSlots { function });
        }
        if candidate.schedule != replayed.schedule {
            return Err(GeneralizedSpillInsertionError::NonCanonicalSchedule { function });
        }
    }
    if plan.usage != expected.usage {
        return Err(GeneralizedSpillInsertionError::UsageMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(GeneralizedSpillInsertionError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let action_count = plan.functions.iter().try_fold(0_usize, |total, function| {
        total
            .checked_add(function.slots.len())
            .ok_or(GeneralizedSpillInsertionError::WorkOverflow)
    })?;
    let event_count = plan.functions.iter().try_fold(0_usize, |total, function| {
        total
            .checked_add(function.schedule.len())
            .ok_or(GeneralizedSpillInsertionError::WorkOverflow)
    })?;
    let receipt = GeneralizedSpillInsertionReceipt {
        identity: generalized_spill_insertion_identity(&plan),
        abstract_spill_insertion: plan.abstract_spill_insertion,
        spill_recovery_actions: plan.spill_recovery_actions,
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
            .map(|function| function.spill_area_bytes)
            .max()
            .unwrap_or(0),
    };
    Ok(ValidatedGeneralizedSpillInsertion { plan, receipt })
}

fn validate_roots(
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    plan: &GeneralizedSpillInsertionPlan,
) -> Result<(), GeneralizedSpillInsertionError> {
    let first_receipt = first.receipt();
    let second_receipt = second.receipt();
    if plan.abstract_spill_insertion != first_receipt.identity()
        || plan.spill_recovery_actions != second_receipt.identity()
        || plan.register_environment != second.plan().register_environment
        || plan.allocator_availability != second.plan().allocator_availability
        || plan.optimization_unit != second.plan().optimization_unit
        || plan.fuel_schedule != second.plan().fuel_schedule
        || second.plan().abstract_spill_insertion != first_receipt.identity()
        || second.plan().register_environment != first_receipt.register_environment()
        || second.plan().allocator_availability != first_receipt.allocator_availability()
        || second.plan().optimization_unit != first_receipt.optimization_unit()
        || second.plan().fuel_schedule != first_receipt.fuel_schedule()
        || plan.functions.len() != first.plan().functions.len()
    {
        return Err(GeneralizedSpillInsertionError::RootMismatch);
    }
    super::compute::admit_policy(plan.policy)
}
