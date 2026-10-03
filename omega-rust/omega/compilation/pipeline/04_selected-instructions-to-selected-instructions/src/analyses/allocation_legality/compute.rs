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
    AllocationLegalityError, ValidatedAllocationLegality, ValidatedAllocatorAvailability,
    ValidatedLiveRanges,
};
use register_homes::AllocationLegalityPlan;

#[cfg(test)]
std::thread_local! {
    pub(crate) static REUSED_FUNCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn compute_terminal_allocation_legality(
    ranges: &ValidatedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    previous: Option<(&ValidatedLiveRanges, &ValidatedAllocationLegality)>,
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
    // A legality function reads its complete live-range function and these
    // immutable roots, never another function. Prior rows are proposals only:
    // independent validation below the analysis entrance still reconstructs
    // every candidate, so stale or corrupted prior facts grant no authority.
    let previous = previous.filter(|(prior_ranges, prior_legality)| {
        prior_ranges.plan().target == ranges.plan().target
            && prior_ranges.plan().optimization_unit == ranges.plan().optimization_unit
            && prior_ranges.plan().fuel_schedule == ranges.plan().fuel_schedule
            && prior_legality.plan().ranges == prior_ranges.receipt().identity()
            && prior_legality.receipt().ranges() == prior_ranges.receipt().identity()
            && prior_legality.plan().register_environment == register_environment
            && prior_legality.receipt().register_environment() == register_environment
            && prior_legality.plan().allocator_availability == availability.receipt().identity()
            && prior_legality.receipt().allocator_availability()
                == availability.receipt().identity()
    });
    let functions = ranges
        .plan()
        .functions
        .iter()
        .enumerate()
        .map(|(function_index, live_ranges)| {
            if let Some((prior_ranges, prior_legality)) = previous
                && prior_ranges.plan().functions.get(function_index) == Some(live_ranges)
                && let Some(function) = prior_legality.plan().functions.get(function_index)
            {
                #[cfg(test)]
                REUSED_FUNCTIONS.set(REUSED_FUNCTIONS.get() + 1);
                return Ok(function.clone());
            }
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
