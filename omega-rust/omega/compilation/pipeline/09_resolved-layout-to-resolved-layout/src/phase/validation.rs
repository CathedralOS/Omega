//! Independent phase-selection, baseline, rewrite, and current-program replay.
use super::{
    OptimizationPhaseSelections, ResolvedLayoutOptimization, ResolvedLayoutOptimizationError,
    ResolvedMachineLayout, ValidatedPhysicalRegisterModel, x86_rel8_selected,
};
use crate::validate_optimized_x86_branch_relaxation;

/// The baseline layout is its own owner's to admit: a reader replaying both
/// calls the resolved-layout validator on `baseline` before this one.
pub fn validate_resolved_layout_optimization(
    physical: &ValidatedPhysicalRegisterModel,
    baseline: &ResolvedMachineLayout,
    selections: &OptimizationPhaseSelections,
    artifact: &ResolvedLayoutOptimization,
) -> Result<(), ResolvedLayoutOptimizationError> {
    if &artifact.selections != selections {
        return Err(ResolvedLayoutOptimizationError::SelectionMismatch);
    }
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    match (enabled, artifact.relaxation.as_ref()) {
        (false, None) => {
            if artifact.layout() != baseline {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        (true, Some(relaxation)) => {
            validate_optimized_x86_branch_relaxation(physical, baseline, relaxation)
                .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
            if artifact.budget != relaxation.budget() || artifact.layout() != relaxation.layout() {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        _ => return Err(ResolvedLayoutOptimizationError::SelectionMismatch),
    }
    Ok(())
}
