use omega_regalloc::{FixedViewCopyError, ValidatedFixedViewCopies, validate_fixed_view_copies};

use crate::{
    StagedOptimizedAllocationLegality, StagedOptimizedAllocationLegalityCustodyReceipt,
    validate_optimized_allocation_legality_custody,
};

use super::custody::fixed_view_copy_custody_receipt;
use super::model::{
    OptimizedFixedViewCopyCustodyError, StagedOptimizedFixedViewCopyCustodyReceipt,
};

pub fn validate_optimized_fixed_view_copy_custody(
    source: &StagedOptimizedAllocationLegality,
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
    source: &StagedOptimizedAllocationLegality,
    copies: &ValidatedFixedViewCopies,
) -> Result<ValidatedFixedViewCopies, FixedViewCopyError> {
    let selected_stage = source.live_range_stage().liveness_stage().selected_stage();
    let environment = selected_stage.register_environment();
    validate_fixed_view_copies(
        selected_stage.selected(),
        source.live_range_stage().ranges(),
        source.legality(),
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        copies.plan().clone(),
    )
}

pub(super) fn validate_source(
    source: &StagedOptimizedAllocationLegality,
) -> Result<StagedOptimizedAllocationLegalityCustodyReceipt, OptimizedFixedViewCopyCustodyError> {
    validate_optimized_allocation_legality_custody(
        source.live_range_stage(),
        source.allocator_availability(),
        source.legality(),
    )
    .map_err(OptimizedFixedViewCopyCustodyError::UpstreamLegality)
}
