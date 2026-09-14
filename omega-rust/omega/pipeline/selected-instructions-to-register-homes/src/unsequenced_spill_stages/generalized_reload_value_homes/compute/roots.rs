//! Producer root and policy admission.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, ValidatedPhysicalRegisterModel,
    ValidatedRegisterConstraintCatalog, ValidatedRegisterReservationProfile,
    target_register_environment_identity,
};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomePolicy,
    ValidatedAbstractSpillInsertion, ValidatedAllocationLegality,
    ValidatedGeneralizedSpillInsertion, ValidatedLiveRanges, ValidatedSpillRecoveryActions,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn admit(
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
    policy: GeneralizedReloadValueHomePolicy,
) -> Result<(), GeneralizedReloadValueHomeError> {
    if policy != GeneralizedReloadValueHomePolicy::EpochZeroAndOneBlockLocalLowestCompatibleViewV1 {
        return Err(GeneralizedReloadValueHomeError::UnsupportedPolicy);
    }
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    let generalized_plan = generalized.plan();
    let second_plan = second.plan();
    let counts = [
        generalized_plan.functions.len(),
        first.plan().functions.len(),
        selected.plan().functions.len(),
        ranges.plan().functions.len(),
        legality.plan().functions.len(),
    ];
    if generalized_plan.abstract_spill_insertion != first.receipt().identity()
        || generalized_plan.spill_recovery_actions != second.receipt().identity()
        || second_plan.abstract_spill_insertion != first.receipt().identity()
        || second_plan.selected != selected.receipt().identity()
        || second_plan.ranges != ranges.receipt().identity()
        || second_plan.legality != legality.receipt().identity()
        || ranges.receipt().selected() != selected.receipt().identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || generalized_plan.register_environment != environment
        || second_plan.register_environment != environment
        || legality.receipt().register_environment() != environment
        || generalized_plan.allocator_availability != legality.receipt().allocator_availability()
        || second_plan.allocator_availability != legality.receipt().allocator_availability()
        || generalized_plan.optimization_unit != selected.receipt().optimization_unit()
        || generalized_plan.optimization_unit != ranges.receipt().optimization_unit()
        || generalized_plan.fuel_schedule != selected.receipt().fuel_schedule()
        || generalized_plan.fuel_schedule != ranges.receipt().fuel_schedule()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || !counts.iter().all(|count| *count == counts[0])
    {
        return Err(GeneralizedReloadValueHomeError::RootMismatch);
    }
    Ok(())
}
