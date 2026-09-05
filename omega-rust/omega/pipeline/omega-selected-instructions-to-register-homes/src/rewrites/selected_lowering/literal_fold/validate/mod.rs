//! Optimizer module role: executable entrance. Independent literal-fold validation entrance.

mod constraints;
mod receipt;
mod replay;
mod roots;
mod usage;

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::{
    LiteralFoldError, LiteralFoldPlan, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
    ValidatedLiteralFold, ValidatedLiveRanges, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

use constraints::reconstruct_immediate_rows;
use receipt::admit_literal_fold;
use replay::reconstruct_literal_fold;
use roots::validate_literal_fold_roots;
use usage::{ensure_budget, reconstruct_fold_usage};

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

    let rows = reconstruct_immediate_rows(constraints, selected_keys, plan.policy)?;
    let (expected_functions, transformed) = reconstruct_literal_fold(selected, recovery, &rows)?;
    if plan.functions != expected_functions {
        return Err(LiteralFoldError::DecisionMismatch { function: 0 });
    }
    let applied_count = expected_functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let usage = reconstruct_fold_usage(selected, applied_count)?;
    if plan.usage != usage {
        return Err(LiteralFoldError::UsageMismatch);
    }
    ensure_budget(plan.usage, plan.budget)?;

    let transformed_selected = selected_instruction_plan_identity(&transformed);
    if plan.transformed_selected != transformed_selected {
        return Err(LiteralFoldError::TransformedIdentityMismatch);
    }
    Ok(admit_literal_fold(
        plan,
        transformed,
        transformed_selected,
        applied_count,
    ))
}
