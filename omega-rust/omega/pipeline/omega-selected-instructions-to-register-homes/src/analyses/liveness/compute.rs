use omega_regalloc::{ValidatedLiveness, analyze_liveness};

use omega_target_operations_to_selected_instructions::StagedOptimizedSelectedInstructions;

use super::model::OptimizedLivenessCustodyError;

pub(super) fn compute_liveness(
    selected: &StagedOptimizedSelectedInstructions,
) -> Result<ValidatedLiveness, OptimizedLivenessCustodyError> {
    analyze_liveness(selected.selected()).map_err(OptimizedLivenessCustodyError::Analysis)
}
