use crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest;
use crate::{PostAllocationSelectedTransformation, assign_register_homes, validate_register_homes};

use crate::{StagedOptimizedAllocationLegality, validate_optimized_allocation_legality_custody};
use crate::{StagedOptimizedSelectedReanalysis, validate_optimized_selected_reanalysis_custody};

use super::custody::{custody_receipt, post_copy_custody_receipt};
use super::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
};

pub(super) fn construct_optimized_register_homes(
    legality: StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let environment = legality.register_environment();
    let homes = assign_register_homes(
        legality.legality(),
        legality.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::Assignment)?;
    construct_with_assignment(legality, homes)
}

pub(super) fn construct_with_assignment(
    legality: StagedOptimizedAllocationLegality,
    homes: crate::ValidatedRegisterHomes,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let upstream = validate_optimized_allocation_legality_custody(
        legality.live_range_stage(),
        legality.allocator_availability(),
        legality.legality(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::UpstreamLegality)?;
    let environment = legality.register_environment();
    let replayed = validate_register_homes(
        legality.legality(),
        legality.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedRegisterHomeCustodyError::Revalidation)?;
    if replayed.receipt() != homes.receipt() {
        return Err(OptimizedRegisterHomeCustodyError::ReceiptMismatch);
    }
    let manifest = project_post_allocation_optimization_manifest(
        upstream.manifest(),
        &[],
        legality.ranges(),
        legality.legality(),
        &homes,
    )
    .map_err(OptimizedRegisterHomeCustodyError::Manifest)?;
    let custody = custody_receipt(upstream, homes.receipt(), manifest.record().identity);
    Ok(StagedOptimizedRegisterHomes {
        legality,
        homes,
        manifest,
        custody,
    })
}

/// Assignment probe over the post-copy reanalysis. Computing the assignment
/// without consuming the reanalysis lets the recovery route answer residual
/// `NoCompatibleHome` pressure with runtime spill instead of post-copy homes.
pub(super) fn assign_optimized_register_homes_after_fixed_view_copies(
    reanalysis: &StagedOptimizedSelectedReanalysis,
) -> Result<crate::ValidatedRegisterHomes, crate::RegisterHomeError> {
    let environment = reanalysis.register_environment();
    assign_register_homes(
        reanalysis.legality(),
        reanalysis.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
    )
}

pub(super) fn construct_optimized_register_homes_after_fixed_view_copies(
    reanalysis: StagedOptimizedSelectedReanalysis,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let homes = assign_optimized_register_homes_after_fixed_view_copies(&reanalysis)
        .map_err(OptimizedPostCopyRegisterHomeCustodyError::Assignment)?;
    construct_optimized_register_homes_after_fixed_view_copies_with_assignment(reanalysis, homes)
}

pub(super) fn construct_optimized_register_homes_after_fixed_view_copies_with_assignment(
    reanalysis: StagedOptimizedSelectedReanalysis,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let source = validate_optimized_selected_reanalysis_custody(
        reanalysis.transformation_stage(),
        reanalysis.liveness(),
        reanalysis.ranges(),
        reanalysis.legality(),
    )
    .map_err(OptimizedPostCopyRegisterHomeCustodyError::UpstreamReanalysis)?;
    let environment = reanalysis.register_environment();
    let replayed = validate_register_homes(
        reanalysis.legality(),
        reanalysis.ranges(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        homes.plan().clone(),
    )
    .map_err(OptimizedPostCopyRegisterHomeCustodyError::Revalidation)?;
    if replayed.receipt() != homes.receipt() {
        return Err(OptimizedPostCopyRegisterHomeCustodyError::ReceiptMismatch);
    }
    let manifest = project_post_allocation_optimization_manifest(
        source.source().manifest(),
        &[PostAllocationSelectedTransformation::FixedViewCopy(
            source.source().transformation(),
        )],
        reanalysis.ranges(),
        reanalysis.legality(),
        &homes,
    )
    .map_err(OptimizedPostCopyRegisterHomeCustodyError::Manifest)?;
    let custody = post_copy_custody_receipt(source, homes.receipt(), manifest.record().identity);
    Ok(StagedOptimizedRegisterHomesAfterFixedViewCopies {
        reanalysis,
        homes,
        manifest,
        custody,
    })
}
