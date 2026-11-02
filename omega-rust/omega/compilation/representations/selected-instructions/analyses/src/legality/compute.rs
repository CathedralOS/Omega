use crate::{ValidatedAllocatorAvailability, analyze_allocation_legality};

use crate::StagedOptimizedLiveRanges;

use super::custody::custody_receipt;
use super::{OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegality};

/// Analyze legality over the staged ranges and the availability the chosen
/// policy just materialized. Both `materialize_allocator_availability` and
/// `analyze_allocation_legality` return only after their own checkers accept
/// the result, so the receipt records those identities without replaying the
/// availability, the legality or the upstream staging chain again.
pub(super) fn compute_allocation_legality(
    ranges: StagedOptimizedLiveRanges,
    availability: ValidatedAllocatorAvailability,
) -> Result<StagedOptimizedAllocationLegality, OptimizedAllocationLegalityCustodyError> {
    let environment = ranges.register_environment();
    let legality = analyze_allocation_legality(
        ranges.ranges(),
        &availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Analysis)?;
    let custody = custody_receipt(
        ranges.custody(),
        availability.receipt().identity(),
        legality.receipt(),
    );
    Ok(StagedOptimizedAllocationLegality {
        ranges,
        availability,
        legality,
        custody,
    })
}
