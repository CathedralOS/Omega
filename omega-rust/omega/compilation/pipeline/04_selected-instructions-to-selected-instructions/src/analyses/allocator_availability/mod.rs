//! Optimizer module role: executable entrance. Explicit allocator-view availability policy entrance.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
pub(crate) mod compute;

use register_homes::AllocatorAvailabilityPolicy;
use register_homes::{AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan};
use register_model::{PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId};

/// Materialize and independently replay one exact named policy controlling
/// unconstrained physical-view availability. This is allocator input only; it
/// grants no fixed-operand override, home assignment, or emission authority.
pub(crate) fn materialize_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: target::NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
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
    seal_allocator_availability(plan)
}

seal_plan!(
    seal_allocator_availability(AllocatorAvailabilityPlan)
        -> ValidatedAllocatorAvailability: AllocatorAvailabilityValidationReceipt
        = register_homes::allocator_availability_identity, AllocatorAvailabilityError
    {
        register_environment,
        physical
    }
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorAvailabilityValidationReceipt {
    pub(crate) identity: AllocatorAvailabilityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: PhysicalRegisterModelIdentity,
}

impl AllocatorAvailabilityValidationReceipt {
    receipt_accessor_rows!(AllocatorAvailabilityValidationReceipt {
        identity: AllocatorAvailabilityIdentity,
        physical: PhysicalRegisterModelIdentity,
        register_environment: TargetRegisterEnvironmentIdentity,
    });
}

validated_plan_carrier!(
    ValidatedAllocatorAvailability: AllocatorAvailabilityPlan,
    AllocatorAvailabilityValidationReceipt
);

impl ValidatedAllocatorAvailability {
    pub(crate) fn unconstrained_views(&self, class: RegisterClassId) -> Option<&[RegisterViewId]> {
        self.plan
            .classes
            .iter()
            .find(|row| row.class == class)
            .map(|row| row.unconstrained_views.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocatorAvailabilityError {
    RootMismatch,
    NonCanonicalAllowlist,
    UnknownView { view: u16 },
    ViewNotEnvironmentAllocatable { view: u16 },
    NonCanonicalPlan,
    PlanMismatch,
}

failed_display!(
    AllocatorAvailabilityError,
    "terminal allocator-availability derivation failed"
);

impl std::error::Error for AllocatorAvailabilityError {}
