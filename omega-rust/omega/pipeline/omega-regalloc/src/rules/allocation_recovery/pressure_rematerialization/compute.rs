//! Rematerialization proposal coordinator: admit custody, derive exact actions,
//! apply them, and bind the canonical transformed identity and work usage.

mod application;
mod candidate_action;
mod function_plans;
mod materialize_constraint;
mod selected_structure;
mod source_custody;
mod work;

use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::{
    PressureRematerializationError, PressureRematerializationPlan, PressureRematerializationPolicy,
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, ValidatedLiveRanges,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

pub(crate) use function_plans::build_functions;
#[cfg(test)]
pub(super) use work::{ensure_budget, required_usage};

#[allow(clippy::too_many_arguments)]
pub(crate) fn compute_terminal_pressure_rematerialization<S: ValidatedSelectedAnalysis>(
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
    policy: PressureRematerializationPolicy,
    budget: OptimizationWorkBudget,
) -> Result<PressureRematerializationPlan, PressureRematerializationError> {
    source_custody::admit(
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
    let materialize = materialize_constraint::select(constraints, selected_keys)?;
    let (functions, transformed) = build_functions(
        selected.selected_plan(),
        ranges.plan(),
        recovery.plan(),
        materialize,
        policy,
    )?;
    let (applied, rewritten_uses) = work::action_counts(&functions)?;
    let usage = work::required_usage(selected.selected_plan(), applied, rewritten_uses)?;
    work::ensure_budget(usage, budget)?;

    Ok(PressureRematerializationPlan {
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
