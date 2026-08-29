use omega_regalloc::{ValidatedLiveness, analyze_liveness};

use crate::StagedOptimizedSelectedInstructions;

use super::model::OptimizedLivenessCustodyError;

pub(super) fn compute_liveness(
    selected: &StagedOptimizedSelectedInstructions,
) -> Result<ValidatedLiveness, OptimizedLivenessCustodyError> {
    analyze_liveness(selected.selected()).map_err(OptimizedLivenessCustodyError::Analysis)
}
