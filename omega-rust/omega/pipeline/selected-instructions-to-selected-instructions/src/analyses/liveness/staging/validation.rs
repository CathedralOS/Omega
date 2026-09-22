use crate::{ValidatedLiveness, validate_liveness};

use target_operations_to_selected_instructions::{
    StagedOptimizedSelectedInstructions, validate_optimized_selection_custody,
};

use super::custody::liveness_custody_receipt;
use super::model::{
    OptimizedLivenessCustodyError, StagedOptimizedLiveness, StagedOptimizedLivenessCustodyReceipt,
};

pub fn validate_optimized_liveness_custody(
    selected: &StagedOptimizedSelectedInstructions,
    liveness: &ValidatedLiveness,
) -> Result<StagedOptimizedLivenessCustodyReceipt, OptimizedLivenessCustodyError> {
    let upstream = validate_optimized_selection_custody(
        selected.optimized_target_owner(),
        selected.register_environment(),
        selected.legalized(),
        selected.selected(),
    )
    .map_err(OptimizedLivenessCustodyError::UpstreamSelection)?;
    let replayed = validate_liveness(selected.selected(), liveness.plan().clone())
        .map_err(OptimizedLivenessCustodyError::Revalidation)?;
    if replayed.receipt() != liveness.receipt() {
        return Err(OptimizedLivenessCustodyError::ReceiptMismatch);
    }
    Ok(liveness_custody_receipt(upstream, replayed.receipt()))
}

/// Re-verify one admitted liveness staging against the retained producer
/// evidence it carries.
pub(crate) fn validate_staged_optimized_liveness_custody(
    liveness: &StagedOptimizedLiveness,
) -> Result<StagedOptimizedLivenessCustodyReceipt, OptimizedLivenessCustodyError> {
    validate_optimized_liveness_custody(liveness.selected_stage(), liveness.liveness())
}
