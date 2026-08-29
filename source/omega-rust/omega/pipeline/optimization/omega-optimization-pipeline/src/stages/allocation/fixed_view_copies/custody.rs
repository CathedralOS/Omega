use omega_regalloc::FixedViewCopyValidationReceipt;

use crate::StagedOptimizedAllocationLegalityCustodyReceipt;

use super::model::StagedOptimizedFixedViewCopyCustodyReceipt;

pub(super) fn fixed_view_copy_custody_receipt(
    upstream: StagedOptimizedAllocationLegalityCustodyReceipt,
    copies: FixedViewCopyValidationReceipt,
) -> StagedOptimizedFixedViewCopyCustodyReceipt {
    StagedOptimizedFixedViewCopyCustodyReceipt {
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
        source_selected: upstream.selected(),
        source_liveness: upstream.liveness(),
        source_ranges: upstream.ranges(),
        source_legality: upstream.legality(),
        transformation: copies.identity(),
        transformed_selected: copies.transformed_selected(),
        policy: copies.policy(),
        usage: copies.usage(),
        function_count: copies.function_count(),
        copy_count: copies.copy_count(),
    }
}
