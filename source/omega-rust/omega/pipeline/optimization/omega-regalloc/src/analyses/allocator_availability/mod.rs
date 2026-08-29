//! Explicit allocator-view availability policy entrance.

use crate::*;

pub(crate) mod compute;
pub(crate) mod identity;
pub(crate) mod model;
pub(crate) mod validate;

pub use identity::allocator_availability_identity;
pub use model::*;
pub use validate::validate_allocator_availability;

/// Materialize and independently replay one exact named policy controlling
/// unconstrained physical-view availability. This is allocator input only; it
/// grants no fixed-operand override, home assignment, or emission authority.
pub fn materialize_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: omega_target::NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: AllocatorAvailabilityPolicy,
) -> Result<ValidatedAllocatorAvailability, AllocatorAvailabilityError> {
    let plan = compute::compute_terminal_allocator_availability(
        register_environment,
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
    )?;
    validate_allocator_availability(
        register_environment,
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan,
    )
}
