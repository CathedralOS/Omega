use crate::{
    AllocatorAvailabilityError, AllocatorAvailabilityValidationReceipt,
    ValidatedAllocatorAvailability,
};
use register_homes::{AllocatorAvailabilityPlan, allocator_availability_identity};

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_allocator_availability(
    plan: AllocatorAvailabilityPlan,
) -> Result<ValidatedAllocatorAvailability, AllocatorAvailabilityError> {
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
