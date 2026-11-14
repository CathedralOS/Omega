/*
  Explicit allocator physical-view availability policy materialization
  (`materialize_allocator_availability`, `ValidatedAllocatorAvailability`).

  Materializes and replays the named `AllocatorAvailabilityPolicy` controlling
  unconstrained physical register view availability for a target register
  environment.
*/

use register_model::{
    PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
    TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
    ValidatedRegisterReservationProfile,
};
pub mod compute {
    use std::collections::BTreeSet;

    use register_model::{
        PreparedRegisterConstraintCatalog, TargetRegisterEnvironmentConstraintKeys,
        TargetRegisterEnvironmentIdentity, ValidatedPhysicalRegisterModel,
        ValidatedRegisterReservationProfile, target_register_environment_identity,
    };
    use target::NativeTarget;

    use crate::AllocatorAvailabilityError;
    use register_homes::{
        AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, RegisterClassAvailability,
    };

    pub(crate) fn compute_terminal_allocator_availability(
        register_environment: TargetRegisterEnvironmentIdentity,
        target: NativeTarget,
        physical: &ValidatedPhysicalRegisterModel,
        constraints: &PreparedRegisterConstraintCatalog,
        reservations: &ValidatedRegisterReservationProfile,
        selected_keys: &TargetRegisterEnvironmentConstraintKeys,
        policy: AllocatorAvailabilityPolicy,
    ) -> Result<AllocatorAvailabilityPlan, AllocatorAvailabilityError> {
        if target.architecture != physical.model().architecture
            || constraints.physical_identity() != physical.identity()
            || reservations.physical_identity() != physical.identity()
            || reservations.target() != target
            || register_environment
                != target_register_environment_identity(
                    target,
                    physical,
                    constraints,
                    reservations,
                    selected_keys,
                )
        {
            return Err(AllocatorAvailabilityError::RootMismatch);
        }
        let admitted = admitted_views(physical, reservations, &policy)?;
        let classes = physical
            .model()
            .classes
            .iter()
            .map(|class| RegisterClassAvailability {
                class: class.id,
                unconstrained_views: class
                    .views
                    .iter()
                    .copied()
                    .filter(|view| admitted.contains(view))
                    .collect(),
            })
            .collect();
        Ok(AllocatorAvailabilityPlan {
            register_environment,
            physical: physical.identity(),
            policy,
            classes,
        })
    }

    fn admitted_views(
        physical: &ValidatedPhysicalRegisterModel,
        reservations: &ValidatedRegisterReservationProfile,
        policy: &AllocatorAvailabilityPolicy,
    ) -> Result<BTreeSet<register_model::RegisterViewId>, AllocatorAvailabilityError> {
        let is_environment_allocatable = |view: &register_model::RegisterView| {
            view.allocatable
                && view
                    .units
                    .iter()
                    .chain(&view.write_units)
                    .all(|unit| !reservations.reserved_units().contains(unit))
        };
        match policy {
            AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1 => Ok(physical
                .model()
                .views
                .iter()
                .filter(|view| is_environment_allocatable(view))
                .map(|view| view.id)
                .collect()),
            AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } => {
                if views.windows(2).any(|pair| pair[0] >= pair[1]) {
                    return Err(AllocatorAvailabilityError::NonCanonicalAllowlist);
                }
                for view_id in views {
                    let Some(view) = physical.model().view(*view_id) else {
                        return Err(AllocatorAvailabilityError::UnknownView { view: view_id.0 });
                    };
                    if !is_environment_allocatable(view) {
                        return Err(AllocatorAvailabilityError::ViewNotEnvironmentAllocatable {
                            view: view_id.0,
                        });
                    }
                }
                Ok(views.iter().copied().collect())
            }
        }
    }
}

use register_homes::AllocatorAvailabilityPolicy;
use register_homes::{AllocatorAvailabilityIdentity, AllocatorAvailabilityPlan};
use register_model::{PhysicalRegisterModelIdentity, RegisterClassId, RegisterViewId};

pub fn materialize_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: target::NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &PreparedRegisterConstraintCatalog,
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
