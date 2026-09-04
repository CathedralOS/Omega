use omega_regalloc::{
    PostAllocationSelectedTransformation, ValidatedPostAllocationOptimizationManifest,
    ValidatedRegisterHomes, validate_post_allocation_optimization_manifest,
    validate_register_homes,
};

use omega_fixed_view_copies_to_reanalyzed_legality::{
    StagedOptimizedSelectedReanalysis, validate_optimized_selected_reanalysis_custody,
};
use omega_live_ranges_to_allocation_legality::{
    StagedOptimizedAllocationLegality, validate_optimized_allocation_legality_custody,
};

use super::custody::{custody_receipt, post_copy_custody_receipt};
use super::model::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
};

pub fn validate_optimized_register_home_custody(
    legality: &StagedOptimizedAllocationLegality,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> Result<StagedOptimizedRegisterHomeCustodyReceipt, OptimizedRegisterHomeCustodyError> {
    let upstream = validate_optimized_allocation_legality_custody(
        legality.live_range_stage(),
        legality.allocator_availability(),
        legality.legality(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::UpstreamLegality)?;
    let ranges = legality.live_range_stage();
    let environment = ranges
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed = validate_register_homes(
        legality.legality(),
        ranges.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::Revalidation)?;
    if replayed.receipt() != homes.receipt() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
    let manifest = validate_post_allocation_optimization_manifest(
        manifest.record(),
        upstream.manifest(),
        &[],
        ranges.ranges(),
        legality.legality(),
        &replayed,
    )
    .map_err(OptimizedRegisterHomeCustodyError::Manifest)?;
    Ok(custody_receipt(
        upstream,
        replayed.receipt(),
        manifest.record().identity,
    ))
}

pub fn validate_optimized_register_home_after_fixed_view_copy_custody(
    reanalysis: &StagedOptimizedSelectedReanalysis,
    homes: &ValidatedRegisterHomes,
    manifest: &ValidatedPostAllocationOptimizationManifest,
) -> Result<
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let source = validate_optimized_selected_reanalysis_custody(
        reanalysis.transformation_stage(),
        reanalysis.liveness(),
        reanalysis.ranges(),
        reanalysis.legality(),
    )
    .map_err(OptimizedPostCopyRegisterHomeCustodyError::UpstreamReanalysis)?;
    let environment = reanalysis
        .transformation_stage()
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let replayed = validate_register_homes(
        reanalysis.legality(),
        reanalysis.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedPostCopyRegisterHomeCustodyError::Revalidation)?;
    if replayed.receipt() != homes.receipt() {
        return Err(OptimizedPostCopyRegisterHomeCustodyError::ReceiptMismatch);
    }
    let manifest = validate_post_allocation_optimization_manifest(
        manifest.record(),
        source.source().manifest(),
        &[PostAllocationSelectedTransformation::FixedViewCopy(
            source.source().transformation(),
        )],
        reanalysis.ranges(),
        reanalysis.legality(),
        &replayed,
    )
    .map_err(OptimizedPostCopyRegisterHomeCustodyError::Manifest)?;
    Ok(post_copy_custody_receipt(
        source,
        replayed.receipt(),
        manifest.record().identity,
    ))
}
