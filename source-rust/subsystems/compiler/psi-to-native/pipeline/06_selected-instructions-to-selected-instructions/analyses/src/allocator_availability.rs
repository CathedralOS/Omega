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
    let receipt = AllocatorAvailabilityValidationReceipt {
        identity: register_homes::allocator_availability_identity(&plan),
        register_environment,
        physical: physical.identity(),
        reservations: reservations.identity(),
    };
    Ok(ValidatedAllocatorAvailability { plan, receipt })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocatorAvailabilityValidationReceipt {
    pub(crate) identity: AllocatorAvailabilityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) physical: PhysicalRegisterModelIdentity,
    pub(crate) reservations: register_model::RegisterReservationProfileIdentity,
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

#[cfg(test)]
mod tests {
    use super::{AllocatorAvailabilityError, materialize_allocator_availability};
    use register_environment::baseline_target_register_environment;
    use register_homes::AllocatorAvailabilityPolicy;
    use target::NativeTarget;

    // Windows ARM64 keeps the Thread Environment Block pointer in x18 and
    // Apple reserves it as the platform register; only Linux ARM64 may
    // allocate it. Both the default policy and an explicit allowlist must agree.
    #[test]
    fn x18_is_allocatable_only_on_linux_arm64() {
        for (target, allocatable) in [
            (NativeTarget::linux_arm64(), true),
            (NativeTarget::macos_arm64(), false),
            (NativeTarget::windows_arm64(), false),
        ] {
            let environment = baseline_target_register_environment(target).unwrap();
            let keys = environment.allocation_constraint_keys();
            let materialize = |policy| {
                materialize_allocator_availability(
                    environment.identity(),
                    target,
                    environment.physical(),
                    environment.constraints(),
                    environment.reservations(),
                    &keys,
                    policy,
                )
            };
            let model = environment.physical().model();
            let x18 = model.view_named("x18").unwrap().id;
            let class = model
                .classes
                .iter()
                .find(|class| class.views.contains(&x18))
                .unwrap()
                .id;

            let all =
                materialize(AllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1).unwrap();
            let views = all.unconstrained_views(class).unwrap();
            assert!(!views.is_empty(), "{target:?}");
            assert_eq!(views.contains(&x18), allocatable, "{target:?}");

            let explicit = materialize(
                AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 {
                    views: vec![x18],
                },
            );
            if allocatable {
                assert!(explicit.is_ok(), "{target:?}");
            } else {
                assert_eq!(
                    explicit.err(),
                    Some(AllocatorAvailabilityError::ViewNotEnvironmentAllocatable { view: x18.0 }),
                    "{target:?}"
                );
            }
        }
    }
}
