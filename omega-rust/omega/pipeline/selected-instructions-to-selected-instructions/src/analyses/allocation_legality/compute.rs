//! Allocation-legality proposal assembly over validated register-model roots.

mod early_clobbers;
mod fixed_views;
mod function;
mod live_points;
mod view_candidates;

#[cfg(test)]
mod tests;

use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    AllocationLegalityError, AllocationLegalityPlan, ValidatedAllocatorAvailability,
    ValidatedLiveRanges,
};

pub(crate) fn compute_terminal_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
) -> Result<AllocationLegalityPlan, AllocationLegalityError> {
    validate_roots(
        ranges,
        availability,
        register_environment,
        physical,
        constraints,
        reservations,
    )?;
    let environment = target_register_environment_identity(
        ranges.plan().target,
        physical,
        constraints,
        reservations,
        selected_keys,
    );
    if environment != register_environment {
        return Err(AllocationLegalityError::RootMismatch);
    }
    let functions = ranges
        .plan()
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, live_ranges)| {
            function::compute(
                function_index,
                live_ranges,
                availability,
                physical,
                reservations,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let structural_unit_functions = ranges
        .plan()
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(function_index, live_ranges)| {
            function::compute(
                function_index,
                live_ranges,
                availability,
                physical,
                reservations,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AllocationLegalityPlan {
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: availability.receipt().identity(),
        functions,
        structural_unit_functions,
    })
}

fn validate_roots(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
) -> Result<(), AllocationLegalityError> {
    let physical_identity = physical.identity();
    if ranges.plan().target.architecture != physical.model().architecture
        || constraints.physical_identity() != physical_identity
        || reservations.physical_identity() != physical_identity
        || reservations.target() != ranges.plan().target
        || availability.receipt().register_environment() != register_environment
        || availability.receipt().physical() != physical_identity
    {
        return Err(AllocationLegalityError::RootMismatch);
    }
    Ok(())
}
