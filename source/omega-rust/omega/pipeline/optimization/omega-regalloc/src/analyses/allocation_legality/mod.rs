//! Per-point physical-view legality compute -> validation entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;

pub use identity::allocation_legality_identity;
pub use model::*;
pub use validate::validate_allocation_legality;

/// Derive exact per-point physical-view candidates and incompatible fixed-view
/// transitions without assigning homes or inserting copies.
pub fn analyze_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedAllocationLegality, AllocationLegalityError> {
    let plan = compute::compute_terminal_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    validate_allocation_legality(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
