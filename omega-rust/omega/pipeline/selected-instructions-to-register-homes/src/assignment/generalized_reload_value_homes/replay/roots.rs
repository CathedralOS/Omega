//! Replay-local source-chain and environment reconstruction.

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
pub(super) fn reconstruct(
    generalized: &ValidatedGeneralizedSpillInsertion,
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: GeneralizedReloadValueHomePolicy,
) -> Result<(), GeneralizedReloadValueHomeError> {
    if !matches!(
        policy,
        GeneralizedReloadValueHomePolicy::EpochZeroAndOneBlockLocalLowestCompatibleViewV1
    ) {
        return Err(GeneralizedReloadValueHomeError::UnsupportedPolicy);
    }
    let rebuilt_environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    let generalized_receipt = generalized.receipt();
    let first_receipt = first.receipt();
    let second_plan = second.plan();
    let range_receipt = ranges.receipt();
    let legality_receipt = legality.receipt();
    let selected_receipt = selected.receipt();
    let lengths_match = generalized.plan().functions.len() == first.plan().functions.len()
        && generalized.plan().functions.len() == selected.plan().functions.len()
        && generalized.plan().functions.len() == ranges.plan().functions.len()
        && generalized.plan().functions.len() == legality.plan().functions.len();
    let identities_match = generalized_receipt.abstract_spill_insertion()
        == first_receipt.identity()
        && generalized_receipt.spill_recovery_actions() == second.receipt().identity()
        && second_plan.abstract_spill_insertion == first_receipt.identity()
        && second_plan.selected == selected_receipt.identity()
        && second_plan.ranges == range_receipt.identity()
        && second_plan.legality == legality_receipt.identity()
        && range_receipt.selected() == selected_receipt.identity()
        && legality_receipt.ranges() == range_receipt.identity();
    let environment_matches = rebuilt_environment == generalized_receipt.register_environment()
        && rebuilt_environment == second_plan.register_environment
        && rebuilt_environment == legality_receipt.register_environment()
        && constraints.physical_identity() == physical.identity()
        && reservations.physical_identity() == physical.identity()
        && reservations.target() == ranges.plan().target;
    let semantic_roots_match = generalized_receipt.allocator_availability()
        == legality_receipt.allocator_availability()
        && second_plan.allocator_availability == legality_receipt.allocator_availability()
        && generalized_receipt.optimization_unit() == selected_receipt.optimization_unit()
        && generalized_receipt.optimization_unit() == range_receipt.optimization_unit()
        && generalized_receipt.fuel_schedule() == selected_receipt.fuel_schedule()
        && generalized_receipt.fuel_schedule() == range_receipt.fuel_schedule();
    if !(lengths_match && identities_match && environment_matches && semantic_roots_match) {
        return Err(GeneralizedReloadValueHomeError::RootMismatch);
    }
    Ok(())
}
