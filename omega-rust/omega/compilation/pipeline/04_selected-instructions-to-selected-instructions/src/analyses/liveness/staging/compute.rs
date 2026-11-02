use crate::{ValidatedLiveness, analyze_liveness};

use crate::SelectedOptimizationInput;

use super::OptimizedLivenessCustodyError;

pub(super) fn compute_liveness(
    selected: &SelectedOptimizationInput,
) -> Result<ValidatedLiveness, OptimizedLivenessCustodyError> {
    analyze_liveness(selected.selected()).map_err(OptimizedLivenessCustodyError::Analysis)
}
