//! Replay-local root and closed-policy reconstruction.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    RecursiveReloadValueHomeError, RecursiveReloadValueHomePolicy, ValidatedAllocationLegality,
    ValidatedGeneralizedReloadValueHomes, ValidatedGeneralizedSpillRecoveryActions,
    ValidatedLiveRanges, ValidatedRecursiveSpillInsertion,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn reconstruct(
    recursive: &ValidatedRecursiveSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
    prior: &ValidatedGeneralizedReloadValueHomes,
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: RecursiveReloadValueHomePolicy,
) -> Result<(), RecursiveReloadValueHomeError> {
    if !matches!(
        policy,
        RecursiveReloadValueHomePolicy::CompleteBlockLocalLowestCompatibleViewV1
    ) {
        return Err(RecursiveReloadValueHomeError::UnsupportedPolicy);
    }
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    let first = recursive.plan();
    let second = recovery.plan();
    let third = prior.plan();
    let same_count = first.functions.len() == third.functions.len()
        && first.functions.len() == selected.plan().functions.len()
        && first.functions.len() == ranges.plan().functions.len()
        && first.functions.len() == legality.plan().functions.len();
    if first.recovery_actions != recovery.receipt().identity()
        || first.generalized_spill_insertion != second.generalized_spill_insertion
        || second.reload_value_homes != prior.receipt().identity()
        || third.generalized_spill_insertion != first.generalized_spill_insertion
        || third.selected != selected.receipt().identity()
        || third.ranges != ranges.receipt().identity()
        || third.legality != legality.receipt().identity()
        || second
            .selected
            .is_some_and(|root| root != selected.receipt().identity())
        || second
            .ranges
            .is_some_and(|root| root != ranges.receipt().identity())
        || ranges.receipt().selected() != selected.receipt().identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || [
            first.register_environment,
            second.register_environment,
            third.register_environment,
            legality.receipt().register_environment(),
        ]
        .into_iter()
        .any(|root| root != environment)
        || [
            first.allocator_availability,
            second.allocator_availability,
            third.allocator_availability,
        ]
        .into_iter()
        .any(|root| root != legality.receipt().allocator_availability())
        || first.optimization_unit != selected.receipt().optimization_unit()
        || first.optimization_unit != ranges.receipt().optimization_unit()
        || first.fuel_schedule != selected.receipt().fuel_schedule()
        || first.fuel_schedule != ranges.receipt().fuel_schedule()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || !same_count
    {
        return Err(RecursiveReloadValueHomeError::RootMismatch);
    }
    Ok(())
}
