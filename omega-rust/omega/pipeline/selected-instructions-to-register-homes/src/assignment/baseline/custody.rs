use crate::RegisterHomeValidationReceipt;
use optimization_core::PostAllocationOptimizationManifestIdentity;

use crate::AllocationLegalityCustodyReceipt;
use crate::SelectedReanalysisCustodyReceipt;

use super::model::{PostCopyRegisterHomeCustodyReceipt, RegisterHomeCustodyReceipt};

pub(super) fn custody_receipt(
    upstream: AllocationLegalityCustodyReceipt,
    homes: RegisterHomeValidationReceipt,
    manifest: PostAllocationOptimizationManifestIdentity,
) -> RegisterHomeCustodyReceipt {
    RegisterHomeCustodyReceipt {
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
    source: SelectedReanalysisCustodyReceipt,
    homes: RegisterHomeValidationReceipt,
    manifest: PostAllocationOptimizationManifestIdentity,
) -> PostCopyRegisterHomeCustodyReceipt {
    PostCopyRegisterHomeCustodyReceipt {
        source,
        homes: homes.identity(),
        post_allocation_manifest: manifest,
        function_count: homes.function_count(),
        assignment_count: homes.assignment_count(),
    }
}
