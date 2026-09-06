//! Optimizer module role: executable entrance. Transition-free physical-home assignment entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod model;
pub(crate) mod validate;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

pub use model::*;
pub use register_homes::{
    FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomeIdentity, RegisterHomePlan,
    VirtualRegisterHome, register_home_identity,
};
pub use validate::validate_register_homes;

/// Assign deterministic physical views for the bounded transition-free,
/// spill-free lane. The result grants no emission or publication authority.
pub fn assign_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<ValidatedRegisterHomes, RegisterHomeError> {
    let plan = compute::compute_terminal_register_homes(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    validate_register_homes(
        legality,
        ranges,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
