//! Exact active-resident rematerialization and replay entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;

pub use identity::pressure_rematerialization_identity;
pub use model::*;
pub use validate::validate_pressure_rematerialization;

/// Insert one value-lineage-only, zero-fuel rematerialization immediately
/// before the supported future-use boundary while retaining the semantic
/// source materialization and its charge.
pub fn rematerialize_selected_active_resident<S: ValidatedSelectedAnalysis>(
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
    budget: omega_optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedPressureRematerialization, PressureRematerializationError> {
    let plan = compute::compute_terminal_pressure_rematerialization(
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
        policy,
        budget,
    )?;
    validate_pressure_rematerialization(
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
        plan,
    )
}
