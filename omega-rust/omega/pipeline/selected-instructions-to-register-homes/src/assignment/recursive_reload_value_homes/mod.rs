//! Optimizer module role: executable entrance. Complete recursive reload-home closure.
//!
//! This join closes every logical reload segment in a validated recursive spill
//! schedule with an independently replayable physical-view assignment. It
//! creates no selected value, instruction, memory effect, frame address, trap,
//! encoding, emission, or publication authority.

mod compute;
mod identity;
mod model;
mod replay;
mod validate;

pub use identity::recursive_reload_value_home_identity;
pub use model::*;
pub use validate::validate_recursive_reload_value_homes;

#[allow(clippy::too_many_arguments)]
pub fn assign_recursive_reload_value_homes(
    recursive: &crate::ValidatedRecursiveSpillInsertion,
    recovery: &crate::ValidatedGeneralizedSpillRecoveryActions,
    prior: &crate::ValidatedGeneralizedReloadValueHomes,
    selected: &target_operations_to_selected_instructions::ValidatedSelectedInstructions,
    ranges: &crate::ValidatedLiveRanges,
    legality: &crate::ValidatedAllocationLegality,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    constraints: &register_model::ValidatedRegisterConstraintCatalog,
    reservations: &register_model::ValidatedRegisterReservationProfile,
    selected_keys: &register_model::TargetRegisterEnvironmentConstraintKeys,
    policy: RecursiveReloadValueHomePolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedRecursiveReloadValueHomes, RecursiveReloadValueHomeError> {
    let plan = compute::compute(
        recursive,
        recovery,
        prior,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
        budget,
    )?;
    validate_recursive_reload_value_homes(
        recursive,
        recovery,
        prior,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
