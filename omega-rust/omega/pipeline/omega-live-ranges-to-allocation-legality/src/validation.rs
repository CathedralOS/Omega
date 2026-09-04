use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedAllocatorAvailability, validate_allocation_legality,
    validate_allocator_availability,
};

use omega_liveness_to_live_ranges::{
    StagedOptimizedLiveRanges, validate_optimized_live_range_custody,
};

use super::custody::custody_receipt;
use super::model::{
    OptimizedAllocationLegalityCustodyError, StagedOptimizedAllocationLegalityCustodyReceipt,
};

pub fn validate_optimized_allocation_legality_custody(
    ranges: &StagedOptimizedLiveRanges,
    availability: &ValidatedAllocatorAvailability,
    legality: &ValidatedAllocationLegality,
) -> Result<StagedOptimizedAllocationLegalityCustodyReceipt, OptimizedAllocationLegalityCustodyError>
{
    let upstream = validate_optimized_live_range_custody(ranges.liveness_stage(), ranges.ranges())
        .map_err(OptimizedAllocationLegalityCustodyError::UpstreamLiveRanges)?;
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed_availability = validate_allocator_availability(
        environment.identity(),
        environment.target(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        availability.plan().clone(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Availability)?;
    if replayed_availability.receipt() != availability.receipt() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    let replayed = validate_allocation_legality(
        ranges.ranges(),
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        legality.plan().clone(),
    )
    .map_err(OptimizedAllocationLegalityCustodyError::Revalidation)?;
    if replayed.receipt() != legality.receipt() {
        return Err(OptimizedAllocationLegalityCustodyError::ReceiptMismatch);
    }
    Ok(custody_receipt(
        upstream,
        availability.receipt().identity(),
        replayed.receipt(),
    ))
}
