use std::collections::BTreeSet;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use target::NativeTarget;

use crate::{
    AllocatorAvailabilityError, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
    RegisterClassAvailability,
};

pub(crate) fn compute_terminal_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
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
                let Some(view) = physical
                    .model()
                    .views
                    .iter()
                    .find(|view| view.id == *view_id)
                else {
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
