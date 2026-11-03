use crate::RegisterHomeValidationReceipt;
use optimization_core::PostAllocationOptimizationManifestIdentity;

use selected_instruction_analyses::StagedOptimizedAllocationLegalityCustodyReceipt;

use super::StagedOptimizedRegisterHomeCustodyReceipt;

pub(super) fn custody_receipt(
    upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
    homes: RegisterHomeValidationReceipt,
    manifest: PostAllocationOptimizationManifestIdentity,
) -> StagedOptimizedRegisterHomeCustodyReceipt {
    StagedOptimizedRegisterHomeCustodyReceipt {
        core: upstream.core(),
        allocator_availability: upstream.allocator_availability(),
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: upstream.ranges(),
        legality: upstream.legality(),
        homes: homes.identity(),
        post_allocation_manifest: manifest,
        function_count: homes.function_count(),
        assignment_count: homes.assignment_count(),
    }
}
