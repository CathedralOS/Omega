use crate::*;
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId,
    TargetRegisterEnvironmentIdentity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorAvailabilityValidationReceipt {
    pub(crate) identity: AllocatorAvailabilityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: PhysicalRegisterModelIdentity,
    pub(crate) class_count: usize,
    pub(crate) unconstrained_view_count: usize,
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

    pub const fn class_count(self) -> usize {
        self.class_count
    }

    pub const fn unconstrained_view_count(self) -> usize {
        self.unconstrained_view_count
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

impl std::fmt::Display for AllocatorAvailabilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "terminal allocator-availability derivation failed: {self:?}"
        )
    }
}

impl std::error::Error for AllocatorAvailabilityError {}
