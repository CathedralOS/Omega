use omega_regalloc::{
    ValidatedAllocationLegality, ValidatedLiveRanges, ValidatedLiveness,
    validate_allocation_legality, validate_live_ranges, validate_liveness,
};

use crate::{
    StagedOptimizedFixedViewCopies, StagedOptimizedFixedViewCopyCustodyReceipt,
    validate_optimized_fixed_view_copy_custody,
};

use super::custody::selected_reanalysis_custody_receipt;
use super::invariants::require_no_transitions;
use super::model::{
    OptimizedSelectedReanalysisError, StagedOptimizedSelectedReanalysisCustodyReceipt,
};

pub fn validate_optimized_selected_reanalysis_custody(
    transformation: &StagedOptimizedFixedViewCopies,
    liveness: &ValidatedLiveness,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
) -> Result<StagedOptimizedSelectedReanalysisCustodyReceipt, OptimizedSelectedReanalysisError> {
    let source = validate_source(transformation)?;
    let replayed_liveness = validate_liveness(transformation.copies(), liveness.plan().clone())
        .map_err(OptimizedSelectedReanalysisError::LivenessRevalidation)?;
    if replayed_liveness.receipt() != liveness.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    let replayed_ranges =
        validate_live_ranges(transformation.copies(), liveness, ranges.plan().clone())
            .map_err(OptimizedSelectedReanalysisError::LiveRangeRevalidation)?;
    if replayed_ranges.receipt() != ranges.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    let environment = transformation
        .source_legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
        .register_environment();
    let availability = transformation
        .source_legality_stage()
        .allocator_availability();
    let replayed_legality = validate_allocation_legality(
        ranges,
        availability,
        environment.identity(),
        environment.physical(),
        environment.constraints(),
        environment.reservations(),
        environment.allocation_constraint_keys(),
        legality.plan().clone(),
    )
    .map_err(OptimizedSelectedReanalysisError::AllocationLegalityRevalidation)?;
    if replayed_legality.receipt() != legality.receipt() {
        return Err(OptimizedSelectedReanalysisError::ReceiptMismatch);
    }
    require_no_transitions(&replayed_legality)?;
    Ok(selected_reanalysis_custody_receipt(
        source,
        &replayed_liveness,
        &replayed_ranges,
        &replayed_legality,
    ))
}

pub(super) fn validate_source(
    transformation: &StagedOptimizedFixedViewCopies,
) -> Result<StagedOptimizedFixedViewCopyCustodyReceipt, OptimizedSelectedReanalysisError> {
    validate_optimized_fixed_view_copy_custody(
        transformation.source_legality_stage(),
        transformation.copies(),
    )
    .map_err(OptimizedSelectedReanalysisError::UpstreamTransformation)
}
