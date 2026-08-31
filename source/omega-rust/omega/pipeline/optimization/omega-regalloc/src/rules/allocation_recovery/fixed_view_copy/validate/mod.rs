//! Optimizer module role: executable entrance. Independently replays fixed-view copy work and seals exact transformed-plan custody.

mod apply;
mod copy_constraint;
mod leaf_destination;
mod roots;
mod shared_entry;
mod transformation;
mod usage;

use copy_constraint::validated_copy_row;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::{
    ValidatedSelectedInstructions, selected_instruction_plan_identity,
};
use roots::validate_roots;
use transformation::replay_transformation;
use usage::replay_usage;

use crate::{
    FixedViewCopyError, FixedViewCopyPlan, FixedViewCopyValidationReceipt,
    ValidatedAllocationLegality, ValidatedFixedViewCopies, ValidatedLiveRanges,
    fixed_view_copy_identity,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_fixed_view_copies(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: FixedViewCopyPlan,
) -> Result<ValidatedFixedViewCopies, FixedViewCopyError> {
    validate_roots(
        selected,
        ranges,
        legality,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        &plan,
    )?;
    let row = validated_copy_row(constraints, selected_keys)?;
    let expected_usage = replay_usage(selected, legality, plan.policy)?;
    if plan.usage != expected_usage {
        return Err(FixedViewCopyError::ReceiptMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(FixedViewCopyError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let (expected_copies, expected_transformed) =
        replay_transformation(selected, legality, selected_keys, row, plan.policy)?;
    if plan.copies != expected_copies {
        let index = plan
            .copies
            .iter()
            .zip(&expected_copies)
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(plan.copies.len().min(expected_copies.len()));
        return Err(FixedViewCopyError::CopyMismatch { index });
    }
    if plan.transformed != expected_transformed {
        return Err(FixedViewCopyError::TransformedPlanMismatch);
    }
    let transformed_selected = selected_instruction_plan_identity(&plan.transformed);
    let receipt = FixedViewCopyValidationReceipt {
        identity: fixed_view_copy_identity(&plan),
        source_selected: plan.source_selected,
        source_ranges: plan.source_ranges,
        source_legality: plan.source_legality,
        register_environment: plan.register_environment,
        allocator_availability: plan.allocator_availability,
        transformed_selected,
        optimization_unit: selected.receipt().optimization_unit(),
        fuel_schedule: selected.receipt().fuel_schedule(),
        policy: plan.policy,
        usage: plan.usage,
        function_count: plan.transformed.functions.len(),
        copy_count: plan.copies.len(),
    };
    Ok(ValidatedFixedViewCopies { plan, receipt })
}
