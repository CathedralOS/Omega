//! Independent replay comparison and generalized reload-home receipt sealing.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomePlan,
    GeneralizedReloadValueHomeReceipt, ValidatedAbstractSpillInsertion,
    ValidatedAllocationLegality, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillInsertion, ValidatedLiveRanges, ValidatedSpillRecoveryActions,
    generalized_reload_value_home_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_generalized_reload_value_homes(
    generalized: &ValidatedGeneralizedSpillInsertion,
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: GeneralizedReloadValueHomePlan,
) -> Result<ValidatedGeneralizedReloadValueHomes, GeneralizedReloadValueHomeError> {
    if plan.generalized_spill_insertion != generalized.receipt().identity()
        || plan.abstract_spill_insertion != first.receipt().identity()
        || plan.spill_recovery_actions != second.receipt().identity()
        || plan.selected != selected.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.register_environment != generalized.receipt().register_environment()
        || plan.allocator_availability != generalized.receipt().allocator_availability()
        || plan.optimization_unit != generalized.receipt().optimization_unit()
        || plan.fuel_schedule != generalized.receipt().fuel_schedule()
    {
        return Err(GeneralizedReloadValueHomeError::RootMismatch);
    }
    let expected = super::replay::replay(
        generalized,
        first,
        second,
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
        return Err(GeneralizedReloadValueHomeError::UsageMismatch);
    }
    if plan.functions.len() != expected.functions.len() {
        return Err(GeneralizedReloadValueHomeError::RootMismatch);
    }
    for (function, (candidate, replayed)) in
        plan.functions.iter().zip(&expected.functions).enumerate()
    {
        if candidate.machine != replayed.machine {
            return Err(GeneralizedReloadValueHomeError::FunctionMismatch { function });
        }
        if candidate.outcomes != replayed.outcomes {
            return Err(GeneralizedReloadValueHomeError::NonCanonicalAssignments { function });
        }
    }
    if !plan.usage.within(plan.budget) {
        return Err(GeneralizedReloadValueHomeError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let assignment_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.outcomes)
        .filter(|outcome| {
            matches!(
                outcome,
                crate::GeneralizedReloadValueHomeOutcome::Assigned(_)
            )
        })
        .count();
    let pressure_count = plan
        .functions
        .iter()
        .flat_map(|function| &function.outcomes)
        .filter(|outcome| {
            matches!(
                outcome,
                crate::GeneralizedReloadValueHomeOutcome::Pressure(_)
            )
        })
        .count();
    let retained_home_count = plan.functions.iter().try_fold(0_usize, |total, function| {
        function.outcomes.iter().try_fold(total, |total, outcome| {
            let count = match outcome {
                crate::GeneralizedReloadValueHomeOutcome::Assigned(assignment) => {
                    assignment.coexisting_homes.len()
                }
                crate::GeneralizedReloadValueHomeOutcome::Pressure(pressure) => {
                    pressure.blocking_homes.len()
                }
            };
            total
                .checked_add(count)
                .ok_or(GeneralizedReloadValueHomeError::WorkOverflow)
        })
    })?;
    let receipt = GeneralizedReloadValueHomeReceipt {
        identity: generalized_reload_value_home_identity(&plan),
        generalized_spill_insertion: plan.generalized_spill_insertion,
        abstract_spill_insertion: plan.abstract_spill_insertion,
        spill_recovery_actions: plan.spill_recovery_actions,
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
        pressure_count,
        retained_home_count,
    };
    Ok(ValidatedGeneralizedReloadValueHomes { plan, receipt })
}
