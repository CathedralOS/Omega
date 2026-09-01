//! Independent replay comparison and complete-home receipt sealing.

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    RecursiveReloadValueHomeError, RecursiveReloadValueHomePlan, RecursiveReloadValueHomeReceipt,
    ValidatedAllocationLegality, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillRecoveryActions, ValidatedLiveRanges,
    ValidatedRecursiveReloadValueHomes, ValidatedRecursiveSpillInsertion,
    recursive_reload_value_home_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_recursive_reload_value_homes(
    recursive: &ValidatedRecursiveSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
    prior: &ValidatedGeneralizedReloadValueHomes,
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: RecursiveReloadValueHomePlan,
) -> Result<ValidatedRecursiveReloadValueHomes, RecursiveReloadValueHomeError> {
    if plan.recursive_spill_insertion != recursive.receipt().identity()
        || plan.recovery_actions != recovery.receipt().identity()
        || plan.prior_reload_value_homes != prior.receipt().identity()
        || plan.selected != selected.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.register_environment != recursive.receipt().register_environment()
        || plan.allocator_availability != recursive.receipt().allocator_availability()
        || plan.optimization_unit != recursive.receipt().optimization_unit()
        || plan.fuel_schedule != recursive.receipt().fuel_schedule()
    {
        return Err(RecursiveReloadValueHomeError::RootMismatch);
    }
    let expected = super::replay::replay(
        recursive,
        recovery,
        prior,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan.policy,
        plan.budget,
    )?;
    if plan.usage != expected.usage {
        return Err(RecursiveReloadValueHomeError::UsageMismatch);
    }
    if plan.functions.len() != expected.functions.len() {
        return Err(RecursiveReloadValueHomeError::RootMismatch);
    }
    for (function, (candidate, replayed)) in
        plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if candidate.machine != replayed.machine {
            return Err(RecursiveReloadValueHomeError::FunctionMismatch { function });
        }
        if candidate.assignments != replayed.assignments {
            return Err(RecursiveReloadValueHomeError::NonCanonicalAssignments { function });
        }
    }
    if !plan.usage.within(plan.budget) {
        return Err(RecursiveReloadValueHomeError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let assignment_count = plan
        .functions
        .iter()
        .map(|function| function.assignments.len())
        .try_fold(0_usize, |total, count| {
            total
                .checked_add(count)
                .ok_or(RecursiveReloadValueHomeError::WorkOverflow)
        })?;
    let retained_home_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.assignments)
        .try_fold(0_usize, |total, row| {
            total
                .checked_add(row.coexisting_homes.len())
                .ok_or(RecursiveReloadValueHomeError::WorkOverflow)
        })?;
    let receipt = RecursiveReloadValueHomeReceipt {
        identity: recursive_reload_value_home_identity(&plan),
        recursive_spill_insertion: plan.recursive_spill_insertion,
        recovery_actions: plan.recovery_actions,
        prior_reload_value_homes: plan.prior_reload_value_homes,
        selected: plan.selected,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        function_count: plan.functions.len(),
        assignment_count,
        retained_home_count,
    };
    Ok(ValidatedRecursiveReloadValueHomes { plan, receipt })
}
