use crate::RegisterHomeValidationReceipt;
use optimization_core::PostAllocationOptimizationManifestIdentity;

use crate::StagedOptimizedAllocationLegalityCustodyReceipt;
use crate::StagedOptimizedSelectedReanalysisCustodyReceipt;

use super::model::{
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
};

pub(super) fn custody_receipt(
    upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
    homes: RegisterHomeValidationReceipt,
    manifest: PostAllocationOptimizationManifestIdentity,
) -> StagedOptimizedRegisterHomeCustodyReceipt {
    StagedOptimizedRegisterHomeCustodyReceipt {
        psi: upstream.psi(),
        target: upstream.target(),
        entry: upstream.entry(),
        optimization: upstream.optimization(),
        projection: upstream.projection(),
        manifest: upstream.manifest(),
        optimization_unit: upstream.optimization_unit(),
        fuel_schedule: upstream.fuel_schedule(),
        register_environment: upstream.register_environment(),
        allocator_availability: upstream.allocator_availability(),
        selected: upstream.selected(),
        liveness: upstream.liveness(),
        ranges: upstream.ranges(),
        legality: upstream.legality(),
        homes: homes.identity(),
        post_allocation_manifest: manifest,
        function_count: homes.function_count(),
        structural_unit_function_count: homes.structural_unit_function_count(),
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
