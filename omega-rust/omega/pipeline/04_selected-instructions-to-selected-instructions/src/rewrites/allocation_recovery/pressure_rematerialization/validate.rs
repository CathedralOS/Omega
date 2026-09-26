//! Independent rematerialization replay coordinator: authenticate custody and
//! decisions, reconstruct the transformed plan, and bind its receipt.

mod application;
mod decision;
mod function_replay;
mod materialize_constraint;
mod receipt;
mod selected_structure;
mod source_custody;
mod work;

#[cfg(test)]
mod tests;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use crate::{
    PressureRematerializationError, PressureRematerializationPlan, ValidatedAllocationLegality,
    ValidatedAllocatorAvailability, ValidatedLiveRanges, ValidatedPressureRematerialization,
    ValidatedRecoveryClassifications, ValidatedSelectedAnalysis, ValidatedSpillChoices,
};

/// Independently authenticates and replays the plain rematerialization recipe.
/// It does not call the proposal builder or accept a decoded artifact as proof.
#[allow(clippy::too_many_arguments)]
pub fn validate_pressure_rematerialization<S: ValidatedSelectedAnalysis>(
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
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    plan: PressureRematerializationPlan,
) -> Result<ValidatedPressureRematerialization, PressureRematerializationError> {
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
        &plan,
    )?;
    let row = materialize_constraint::select(constraints, selected_keys)?;
    let replay = function_replay::reconstruct(selected, ranges, recovery, &plan, row)?;
    let usage = work::independent_usage(selected, replay.applied, replay.rewritten_uses)?;
    work::validate(usage, plan.usage, plan.budget)?;

    let transformed_selected = selected_instruction_plan_identity(&replay.transformed);
    if plan.transformed_selected != transformed_selected {
        return Err(PressureRematerializationError::TransformedIdentityMismatch);
    }
    let validation_receipt = receipt::bind(
        &plan,
        transformed_selected,
        replay.transformed.functions.len(),
        replay.applied,
        replay.rewritten_uses,
    );
    Ok(ValidatedPressureRematerialization {
        plan,
        transformed: replay.transformed.into(),
        receipt: validation_receipt,
    })
}
