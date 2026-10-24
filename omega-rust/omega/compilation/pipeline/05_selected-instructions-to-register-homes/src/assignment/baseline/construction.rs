use crate::assign_register_homes;
use crate::assignment::post_allocation_manifest::project_post_allocation_optimization_manifest;
use register_homes::PostAllocationSelectedTransformation;

use selected_instructions_to_selected_instructions::{
    StagedOptimizedAllocationLegality, StagedOptimizedSelectedReanalysis,
};

use super::custody::{custody_receipt, post_copy_custody_receipt};
use super::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
};

pub(super) fn construct_with_assignment(
    legality: StagedOptimizedAllocationLegality,
    homes: crate::ValidatedRegisterHomes,
) -> Result<StagedOptimizedRegisterHomes, OptimizedRegisterHomeCustodyError> {
    let upstream = legality.custody();
    let manifest = project_post_allocation_optimization_manifest(
        upstream.manifest(),
        None,
        None,
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

pub(super) fn construct_optimized_register_homes_after_fixed_view_copies_with_assignment(
    reanalysis: StagedOptimizedSelectedReanalysis,
    homes: crate::ValidatedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomesAfterFixedViewCopies,
    OptimizedPostCopyRegisterHomeCustodyError,
> {
    let source = reanalysis.custody();
    let _environment = reanalysis.register_environment();
    let manifest = project_post_allocation_optimization_manifest(
        source.source().manifest(),
        None,
        None,
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
