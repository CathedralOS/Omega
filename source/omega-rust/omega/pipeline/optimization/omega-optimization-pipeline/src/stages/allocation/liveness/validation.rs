use omega_regalloc::{ValidatedLiveness, validate_liveness};

use crate::{StagedOptimizedSelectedInstructions, validate_optimized_selection_custody};

use super::custody::liveness_custody_receipt;
use super::model::{OptimizedLivenessCustodyError, StagedOptimizedLivenessCustodyReceipt};

pub fn validate_optimized_liveness_custody(
    selected: &StagedOptimizedSelectedInstructions,
    liveness: &ValidatedLiveness,
) -> Result<StagedOptimizedLivenessCustodyReceipt, OptimizedLivenessCustodyError> {
    let upstream = validate_optimized_selection_custody(
        selected.optimized_target(),
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
