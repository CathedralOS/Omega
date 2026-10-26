use crate::AllocationLegalityValidationReceipt;
use register_homes::AllocatorAvailabilityIdentity;

use crate::StagedOptimizedLiveRangeCustodyReceipt;

use super::StagedOptimizedAllocationLegalityCustodyReceipt;

pub(super) fn custody_receipt(
    upstream: StagedOptimizedLiveRangeCustodyReceipt,
    allocator_availability: AllocatorAvailabilityIdentity,
    legality: AllocationLegalityValidationReceipt,
) -> StagedOptimizedAllocationLegalityCustodyReceipt {
    StagedOptimizedAllocationLegalityCustodyReceipt {
        core: upstream.core(),
        allocator_availability,
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: upstream.ranges(),
        legality: legality.identity(),
    }
}
