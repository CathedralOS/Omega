use crate::RegisterHomeValidationReceipt;
use optimization_core::PostAllocationOptimizationManifestIdentity;

use selected_instructions_to_selected_instructions::{
    StagedOptimizedAllocationLegalityCustodyReceipt,
    StagedOptimizedSelectedReanalysisCustodyReceipt,
};

use super::{
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
};

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

pub(super) fn post_copy_custody_receipt(
    source: StagedOptimizedSelectedReanalysisCustodyReceipt,
    homes: RegisterHomeValidationReceipt,
    manifest: PostAllocationOptimizationManifestIdentity,
) -> StagedOptimizedPostCopyRegisterHomeCustodyReceipt {
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt {
        source,
        homes: homes.identity(),
        post_allocation_manifest: manifest,
        function_count: homes.function_count(),
        assignment_count: homes.assignment_count(),
    }
}
