use std::collections::BTreeSet;

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};
use omega_target::NativeTarget;

use crate::{
    TerminalAllocatorAvailabilityError, TerminalAllocatorAvailabilityPlan,
    TerminalAllocatorAvailabilityPolicy, TerminalRegisterClassAvailability,
};

pub(crate) fn compute_terminal_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: TerminalAllocatorAvailabilityPolicy,
) -> Result<TerminalAllocatorAvailabilityPlan, TerminalAllocatorAvailabilityError> {
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
        return Err(TerminalAllocatorAvailabilityError::RootMismatch);
    }
    let admitted = admitted_views(physical, reservations, &policy)?;
    let classes = physical
        .model()
        .classes
        .iter()
        .map(|class| TerminalRegisterClassAvailability {
            class: class.id,
            unconstrained_views: class
                .views
                .iter()
                .copied()
                .filter(|view| admitted.contains(view))
                .collect(),
        })
        .collect();
    Ok(TerminalAllocatorAvailabilityPlan {
        register_environment,
        physical: physical.identity(),
        policy,
        classes,
    })
}

fn admitted_views(
    physical: &ValidatedPhysicalRegisterModel,
    reservations: &ValidatedRegisterReservationProfile,
    policy: &TerminalAllocatorAvailabilityPolicy,
) -> Result<BTreeSet<omega_register_model::RegisterViewId>, TerminalAllocatorAvailabilityError> {
    let is_environment_allocatable = |view: &omega_register_model::RegisterView| {
        view.allocatable
            && view
                .units
                .iter()
                .chain(&view.write_units)
                .all(|unit| !reservations.reserved_units().contains(unit))
    };
    match policy {
        TerminalAllocatorAvailabilityPolicy::AllEnvironmentAllocatableViewsV1 => Ok(physical
            .model()
            .views
            .iter()
            .filter(|view| is_environment_allocatable(view))
            .map(|view| view.id)
            .collect()),
        TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } => {
            if views.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(TerminalAllocatorAvailabilityError::NonCanonicalAllowlist);
            }
            for view_id in views {
                let Some(view) = physical
                    .model()
                    .views
                    .iter()
                    .find(|view| view.id == *view_id)
                else {
                    return Err(TerminalAllocatorAvailabilityError::UnknownView {
                        view: view_id.0,
                    });
                };
                if !is_environment_allocatable(view) {
                    return Err(
                        TerminalAllocatorAvailabilityError::ViewNotEnvironmentAllocatable {
                            view: view_id.0,
                        },
                    );
                }
            }
            Ok(views.iter().copied().collect())
        }
    }
}
