//! Optimizer module role: executable entrance. Explicit allocator-view availability policy entrance.

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
pub(crate) mod compute;
pub(crate) mod validate;

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
    validate::seal_allocator_availability(plan)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorAvailabilityValidationReceipt {
    pub(crate) identity: AllocatorAvailabilityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: PhysicalRegisterModelIdentity,
}

impl AllocatorAvailabilityValidationReceipt {
    pub const fn identity(self) -> AllocatorAvailabilityIdentity {
        self.identity
    }

    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }

    pub const fn physical(self) -> PhysicalRegisterModelIdentity {
        self.physical
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAllocatorAvailability {
    pub(crate) plan: AllocatorAvailabilityPlan,
    pub(crate) receipt: AllocatorAvailabilityValidationReceipt,
}

impl ValidatedAllocatorAvailability {
    pub const fn plan(&self) -> &AllocatorAvailabilityPlan {
        &self.plan
    }

    pub const fn receipt(&self) -> AllocatorAvailabilityValidationReceipt {
        self.receipt
    }

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
