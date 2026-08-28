use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::{
    LiteralFoldError, LiteralFoldPlan, LiteralFoldValidationReceipt, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiteralFold, ValidatedLiveRanges,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
    literal_fold_identity,
    literal_fold_transform::{
        ensure_budget, fold_usage, immediate_rows, replay_actions, validate_literal_fold_roots,
    },
};

#[allow(clippy::too_many_arguments)]
pub fn validate_literal_fold<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: LiteralFoldPlan,
) -> Result<ValidatedLiteralFold, LiteralFoldError> {
    validate_literal_fold_roots(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    if plan.source_selected != selected.selected_identity()
        || plan.spill_choices != spill_choices.receipt().identity()
        || plan.recovery_classifications != recovery.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.legality != legality.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != availability.receipt().identity()
        || plan.optimization_unit != selected.optimization_unit_identity()
        || plan.fuel_schedule != selected.fuel_schedule_identity()
    {
        return Err(LiteralFoldError::RootMismatch);
    }
    let rows = immediate_rows(constraints, selected_keys, plan.policy)?;
    let (expected_functions, transformed) = replay_actions(selected, recovery, &rows)?;
    if plan.functions != expected_functions {
        return Err(LiteralFoldError::DecisionMismatch { function: 0 });
    }
    let applied_count = expected_functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let usage = fold_usage(selected, applied_count)?;
    if plan.usage != usage {
        return Err(LiteralFoldError::UsageMismatch);
    }
    ensure_budget(plan.usage, plan.budget)?;
    let transformed_selected = selected_instruction_plan_identity(&transformed);
    if plan.transformed_selected != transformed_selected {
        return Err(LiteralFoldError::TransformedIdentityMismatch);
    }
    let receipt = LiteralFoldValidationReceipt {
        identity: literal_fold_identity(&plan),
        source_selected: plan.source_selected,
        spill_choices: plan.spill_choices,
        recovery_classifications: plan.recovery_classifications,
        ranges: plan.ranges,
        legality: plan.legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        optimization_unit: plan.optimization_unit,
        fuel_schedule: plan.fuel_schedule,
        transformed_selected,
        policy: plan.policy,
        usage: plan.usage,
        function_count: transformed.functions.len(),
        applied_count,
    };
    Ok(ValidatedLiteralFold {
        plan,
        transformed,
        receipt,
    })
}
