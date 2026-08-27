use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_target::NativeTarget;

use crate::{
    TerminalAllocatorAvailabilityError, TerminalAllocatorAvailabilityPlan,
    TerminalAllocatorAvailabilityPolicy, TerminalAllocatorAvailabilityValidationReceipt,
    ValidatedTerminalAllocatorAvailability, terminal_allocator_availability_identity,
};

pub fn validate_terminal_allocator_availability(
    register_environment: TargetRegisterEnvironmentIdentity,
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: TerminalAllocatorAvailabilityPlan,
) -> Result<ValidatedTerminalAllocatorAvailability, TerminalAllocatorAvailabilityError> {
    if plan.register_environment != register_environment || plan.physical != physical.identity() {
        return Err(TerminalAllocatorAvailabilityError::RootMismatch);
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
        return Err(TerminalAllocatorAvailabilityError::NonCanonicalPlan);
    }
    let expected = crate::allocator_availability_compute::compute_terminal_allocator_availability(
        register_environment,
        target,
        physical,
        constraints,
        reservations,
        selected_keys,
        plan.policy.clone(),
    )?;
    if plan != expected {
        return Err(TerminalAllocatorAvailabilityError::PlanMismatch);
    }
    let receipt = TerminalAllocatorAvailabilityValidationReceipt {
        identity: terminal_allocator_availability_identity(&plan),
        register_environment: plan.register_environment,
        physical: plan.physical,
        class_count: plan.classes.len(),
        unconstrained_view_count: plan
            .classes
            .iter()
            .map(|row| row.unconstrained_views.len())
            .sum(),
    };
    Ok(ValidatedTerminalAllocatorAvailability { plan, receipt })
}

fn validate_canonical_policy(
    policy: &TerminalAllocatorAvailabilityPolicy,
) -> Result<(), TerminalAllocatorAvailabilityError> {
    if let TerminalAllocatorAvailabilityPolicy::ExplicitUnconstrainedViewAllowlistV1 { views } =
        policy
        && views.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(TerminalAllocatorAvailabilityError::NonCanonicalAllowlist);
    }
    Ok(())
}
