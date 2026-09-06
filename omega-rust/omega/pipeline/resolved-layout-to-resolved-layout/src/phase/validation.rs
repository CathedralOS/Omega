//! Independent phase-selection, baseline, rewrite, and current-program replay.

use super::*;
use crate::validate_optimized_x86_branch_relaxation;

#[allow(clippy::too_many_arguments)]
pub fn validate_resolved_layout_optimization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    optimization: Option<&StagedOptimizedPostAllocationMachineOptimization>,
    baseline: &StagedOptimizedResolvedSelectedFormLayout,
    selections: &OptimizationPhaseSelections,
    artifact: &ResolvedLayoutOptimization,
) -> Result<(), ResolvedLayoutOptimizationError> {
    validate_optimized_resolved_selected_form_layout_with_post_allocation_machine_optimization(
        selected,
        machine,
        physical,
        encoding,
        optimization,
        baseline,
    )
    .map_err(ResolvedLayoutOptimizationError::Baseline)?;
    if &artifact.selections != selections {
        return Err(ResolvedLayoutOptimizationError::SelectionMismatch);
    }
    let enabled = x86_rel8_selected(selections, baseline.target().architecture)
        .map_err(ResolvedLayoutOptimizationError::Catalog)?;
    match (enabled, artifact.relaxation.as_ref()) {
        (false, None) => {
            if artifact.layout() != baseline.program() {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        (true, Some(relaxation)) => {
            if optimization.is_some() {
                return Err(ResolvedLayoutOptimizationError::UnsupportedComposition);
            }
            validate_optimized_x86_branch_relaxation(
                selected, machine, physical, encoding, baseline, relaxation,
            )
            .map_err(ResolvedLayoutOptimizationError::Relaxation)?;
            if artifact.budget != relaxation.budget() || artifact.layout() != relaxation.layout() {
                return Err(ResolvedLayoutOptimizationError::CurrentProgramMismatch);
            }
        }
        _ => return Err(ResolvedLayoutOptimizationError::SelectionMismatch),
    }
    Ok(())
}
