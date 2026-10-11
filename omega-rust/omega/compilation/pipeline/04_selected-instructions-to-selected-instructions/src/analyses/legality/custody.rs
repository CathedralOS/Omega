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
        function_count: legality.function_count(),
        virtual_register_count: legality.virtual_register_count(),
        point_count: legality.point_count(),
        candidate_count: legality.candidate_count(),
        entry_transition_count: legality.entry_transition_count(),
    }
}
