use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target::NativeTarget;

use crate::{
    AllocatorAvailabilityError, AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy,
    AllocatorAvailabilityValidationReceipt, ValidatedAllocatorAvailability,
    allocator_availability_identity,
};

pub fn validate_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: AllocatorAvailabilityPlan,
) -> Result<ValidatedAllocatorAvailability, AllocatorAvailabilityError> {
    if plan.register_environment != register_environment || plan.physical != physical.identity() {
        return Err(AllocatorAvailabilityError::RootMismatch);
    }
    validate_canonical_policy(&plan.policy)?;
    if plan
        .classes
        .windows(2)
        .any(|pair| pair[0].class >= pair[1].class)
        || plan.classes.iter().any(|row| {
            row.unconstrained_views
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        })
    {
        return Err(AllocatorAvailabilityError::NonCanonicalPlan);
    }
    let expected =
        crate::analyses::allocator_availability::compute::compute_terminal_allocator_availability(
            register_environment,
            target,
            physical,
            constraints,
            reservations,
            selected_keys,
            plan.policy.clone(),
        )?;
    if plan != expected {
        return Err(AllocatorAvailabilityError::PlanMismatch);
    }
    let receipt = AllocatorAvailabilityValidationReceipt {
        identity: allocator_availability_identity(&plan),
        register_environment: plan.register_environment,
        physical: plan.physical,
        class_count: plan.classes.len(),
        unconstrained_view_count: plan
            .classes
            .iter()
            .map(|row| row.unconstrained_views.len())
            .sum(),
    };
    Ok(ValidatedAllocatorAvailability { plan, receipt })
}

fn validate_canonical_policy(
    policy: &AllocatorAvailabilityPolicy,
) -> Result<(), AllocatorAvailabilityError> {
    if let AllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } = policy
        && views.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(AllocatorAvailabilityError::NonCanonicalAllowlist);
    }
    Ok(())
}
