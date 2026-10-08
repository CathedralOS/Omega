use selected_instructions::SelectedInstructionPlan;

use super::super::{OptimizedResolvedSelectedFormLayoutError, SelectedFunctionLayoutPolicy};

/// The validator re-derives the producer's policy through the shared
/// selection: the ordinary directory owns the policy vocabulary, so the
/// same function answers both sides rather than restating it here.
pub(super) fn derive(
    selected: &SelectedInstructionPlan,
) -> Result<SelectedFunctionLayoutPolicy, OptimizedResolvedSelectedFormLayoutError> {
    super::super::ordinary::select(selected)
}
