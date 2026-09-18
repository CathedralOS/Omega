use crate::{FixedViewCopyError, ValidatedFixedViewCopies, validate_fixed_view_copies};

use crate::{
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    StagedOptimizedFixedPrecoloredSegmentHomes,
    validate_optimized_fixed_precolored_segment_home_custody,
};

use super::custody::fixed_view_copy_custody_receipt;
use super::model::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopyCustodyReceipt,
};

pub fn validate_optimized_fixed_view_copy_custody(
    source: &StagedOptimizedFixedPrecoloredSegmentHomes,
    copies: &ValidatedFixedViewCopies,
) -> Result<StagedOptimizedFixedViewCopyCustodyReceipt, OptimizedFixedViewCopyCustodyError> {
    let upstream = validate_source(source)?;
    let replayed =
        revalidate(source, copies).map_err(OptimizedFixedViewCopyCustodyError::Revalidation)?;
    if replayed.receipt() != copies.receipt() {
        return Err(OptimizedFixedViewCopyCustodyError::ReceiptMismatch);
    }
    Ok(fixed_view_copy_custody_receipt(
        upstream,
        replayed.receipt(),
    ))
}

fn revalidate(
    source: &StagedOptimizedFixedPrecoloredSegmentHomes,
    copies: &ValidatedFixedViewCopies,
) -> Result<ValidatedFixedViewCopies, FixedViewCopyError> {
    let environment = source.register_environment();
    validate_fixed_view_copies(
        source.selected(),
        source.ranges(),
        source.legality(),
        source.fixed_intervals(),
        source.split_requirements(),
        source.segment_homes(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        &environment.allocation_constraint_keys(),
        copies.plan().clone(),
    )
}

pub(super) fn validate_source(
    source: &StagedOptimizedFixedPrecoloredSegmentHomes,
) -> Result<
    StagedOptimizedFixedPrecoloredSegmentHomeCustodyReceipt,
    OptimizedFixedViewCopyCustodyError,
> {
    validate_optimized_fixed_precolored_segment_home_custody(
        source.source_legality_stage(),
        source.fixed_intervals(),
        source.split_requirements(),
        source.segment_homes(),
    )
    .map_err(OptimizedFixedViewCopyCustodyError::UpstreamSegmentHomes)
}
