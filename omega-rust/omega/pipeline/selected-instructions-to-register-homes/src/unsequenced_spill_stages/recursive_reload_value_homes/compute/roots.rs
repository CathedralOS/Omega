//! Producer root and closed-policy admission.

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
pub(super) fn admit(
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
    if policy != RecursiveReloadValueHomePolicy::CompleteBlockLocalLowestCompatibleViewV1 {
        return Err(RecursiveReloadValueHomeError::UnsupportedPolicy);
    }
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    let recursive_plan = recursive.plan();
    let recovery_plan = recovery.plan();
    let prior_plan = prior.plan();
    let counts = [
        recursive_plan.functions.len(),
        prior_plan.functions.len(),
        selected.plan().functions.len(),
        ranges.plan().functions.len(),
        legality.plan().functions.len(),
    ];
    if recursive_plan.recovery_actions != recovery.receipt().identity()
        || recursive_plan.generalized_spill_insertion != recovery_plan.generalized_spill_insertion
        || recovery_plan.reload_value_homes != prior.receipt().identity()
        || prior_plan.generalized_spill_insertion != recursive_plan.generalized_spill_insertion
        || prior_plan.selected != selected.receipt().identity()
        || prior_plan.ranges != ranges.receipt().identity()
        || prior_plan.legality != legality.receipt().identity()
        || recovery_plan
            .selected
            .is_some_and(|root| root != selected.receipt().identity())
        || recovery_plan
            .ranges
            .is_some_and(|root| root != ranges.receipt().identity())
        || ranges.receipt().selected() != selected.receipt().identity()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || recursive_plan.register_environment != environment
        || recovery_plan.register_environment != environment
        || prior_plan.register_environment != environment
        || legality.receipt().register_environment() != environment
        || recursive_plan.allocator_availability != legality.receipt().allocator_availability()
        || recovery_plan.allocator_availability != legality.receipt().allocator_availability()
        || prior_plan.allocator_availability != legality.receipt().allocator_availability()
        || recursive_plan.optimization_unit != selected.receipt().optimization_unit()
        || recursive_plan.optimization_unit != ranges.receipt().optimization_unit()
        || recursive_plan.fuel_schedule != selected.receipt().fuel_schedule()
        || recursive_plan.fuel_schedule != ranges.receipt().fuel_schedule()
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || !counts.iter().all(|count| *count == counts[0])
    {
        return Err(RecursiveReloadValueHomeError::RootMismatch);
    }
    Ok(())
}
