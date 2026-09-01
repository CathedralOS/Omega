//! Independent replay comparison and epoch-two choice receipt sealing.

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
};

use crate::{
    GeneralizedSpillRecoveryChoiceError, GeneralizedSpillRecoveryChoicePlan,
    GeneralizedSpillRecoveryChoiceReceipt, ValidatedAllocationLegality,
    ValidatedGeneralizedReloadValueHomes, ValidatedGeneralizedSpillRecoveryChoices,
    ValidatedGeneralizedSpillRecoveryWorklist, generalized_spill_recovery_choice_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_generalized_spill_recovery_choices(
    worklist: &ValidatedGeneralizedSpillRecoveryWorklist,
    homes: &ValidatedGeneralizedReloadValueHomes,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: GeneralizedSpillRecoveryChoicePlan,
) -> Result<ValidatedGeneralizedSpillRecoveryChoices, GeneralizedSpillRecoveryChoiceError> {
    let source = worklist.plan();
    let home = homes.receipt();
    if plan.worklist != worklist.receipt().identity()
        || plan.reload_value_homes != home.identity()
        || plan.legality != home.legality()
        || plan.register_environment != home.register_environment()
        || plan.allocator_availability != home.allocator_availability()
        || plan.optimization_unit != home.optimization_unit()
        || plan.fuel_schedule != home.fuel_schedule()
        || source.legality != plan.legality
        || source.register_environment != plan.register_environment
        || source.allocator_availability != plan.allocator_availability
    {
        return Err(GeneralizedSpillRecoveryChoiceError::RootMismatch);
    }
    let expected = super::replay::replay(
        worklist,
        homes,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan.policy,
        plan.budget,
    )?;
    if plan.usage != expected.usage {
        return Err(GeneralizedSpillRecoveryChoiceError::UsageMismatch);
    }
    if plan.choices != expected.choices {
        return Err(GeneralizedSpillRecoveryChoiceError::NonCanonicalChoices);
    }
    if !plan.usage.within(plan.budget) {
        return Err(GeneralizedSpillRecoveryChoiceError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let contender_count = plan.choices.iter().try_fold(0_usize, |total, choice| {
        total
            .checked_add(choice.contenders.len())
            .ok_or(GeneralizedSpillRecoveryChoiceError::WorkOverflow)
    })?;
    let receipt = GeneralizedSpillRecoveryChoiceReceipt {
        identity: generalized_spill_recovery_choice_identity(&plan),
        worklist: plan.worklist,
        reload_value_homes: plan.reload_value_homes,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        usage: plan.usage,
        choice_count: plan.choices.len(),
        contender_count,
    };
    Ok(ValidatedGeneralizedSpillRecoveryChoices { plan, receipt })
}
