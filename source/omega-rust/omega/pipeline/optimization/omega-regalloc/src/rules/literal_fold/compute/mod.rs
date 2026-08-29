//! Literal-fold proposal coordination entrance.

mod actions;
mod constraints;
mod function_rewrite;
mod functions;
mod roots;
mod usage;

use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::{
    LiteralFoldError, LiteralFoldPlan, LiteralFoldPolicy, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

use constraints::select_immediate_rows;
use functions::derive_function_folds;
use roots::validate_literal_fold_roots;
use usage::{ensure_budget, fold_usage};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_literal_fold<S: ValidatedSelectedAnalysis>(
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
    policy: LiteralFoldPolicy,
    budget: OptimizationWorkBudget,
) -> Result<LiteralFoldPlan, LiteralFoldError> {
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
    let rows = select_immediate_rows(constraints, selected_keys, policy)?;
    let (functions, transformed) = derive_function_folds(selected, recovery, &rows)?;
    let applied = functions
        .iter()
        .filter(|function| function.action.is_some())
        .count();
    let usage = fold_usage(selected, applied)?;
    ensure_budget(usage, budget)?;

    Ok(LiteralFoldPlan {
        source_selected: selected.selected_identity(),
        spill_choices: spill_choices.receipt().identity(),
        recovery_classifications: recovery.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        policy,
        budget,
        usage,
        functions,
        transformed_selected: selected_instruction_plan_identity(&transformed),
    })
}
