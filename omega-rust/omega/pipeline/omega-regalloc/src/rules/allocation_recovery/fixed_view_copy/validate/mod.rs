//! Optimizer module role: executable entrance. Independently replays fixed-view copy work and seals exact transformed-plan custody.

mod apply;
mod copy_constraint;
mod leaf_destination;
mod roots;
mod seal;
mod shared_entry;
mod source_evidence;
mod transformation;
mod usage;

use copy_constraint::validated_copy_row;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;
use roots::validate_roots;
use seal::seal_validation;
use transformation::replay_transformation;
use usage::replay_usage;

use crate::{
    FixedViewCopyError, FixedViewCopyPlan, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSegmentHomes,
    ValidatedFixedPrecoloredSplitRequirements, ValidatedFixedViewCopies, ValidatedLiveRanges,
};

#[allow(clippy::too_many_arguments)]
pub fn validate_fixed_view_copies(
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    homes: &ValidatedFixedPrecoloredSegmentHomes,
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
    let evidence = source_evidence::reconstruct(
        ranges,
        legality,
        fixed,
        requirements,
        homes,
        plan.source_evidence,
    )?;
    let row = validated_copy_row(constraints, selected_keys)?;
    let expected_usage = super::work::combined_usage(
        evidence.usage,
        replay_usage(selected, &evidence.boundaries, plan.policy)?,
    )?;
    if plan.usage != expected_usage {
        return Err(FixedViewCopyError::ReceiptMismatch);
    }
    if !plan.usage.within(plan.budget) {
        return Err(FixedViewCopyError::BudgetExceeded {
            required: plan.usage,
            budget: plan.budget,
        });
    }
    let (expected_copies, expected_transformed) = replay_transformation(
        selected,
        &evidence.boundaries,
        selected_keys,
        row,
        plan.policy,
    )?;
    if plan.copies != expected_copies {
        let index = plan
            .copies
            .iter()
            .zip(&expected_copies)
            .position(|(actual, expected)| actual != expected)
            .unwrap_or(plan.copies.len().min(expected_copies.len()));
        return Err(FixedViewCopyError::CopyMismatch { index });
    }
    if *plan.transformed != expected_transformed {
        return Err(FixedViewCopyError::TransformedPlanMismatch);
    }
    Ok(seal_validation(selected, plan))
}
