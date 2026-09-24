use super::super::error::FunctionRelativeOptimizationRealizationError;
use optimization_core::{OptimizationExecutionPhase, OptimizationSelections};
use resolved_layout_to_resolved_layout::ResolvedLayoutOptimization;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

/// Manifest construction consumes current data and already-replayed phase
/// evidence. It does not select a layout from optimization history.
pub(in crate::function_realization) fn validate_layout_optimization_manifest_roots(
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    if layout.selections() != &phase {
        return Err(FunctionRelativeOptimizationRealizationError::RootMismatch);
    }
    match layout.relaxation() {
        None if layout.layout() == baseline_layout.program() => Ok(()),
        Some(relaxation)
            if relaxation.source() == baseline_layout.identity()
                && relaxation.output() == layout.layout().identity()
                && relaxation.layout() == layout.layout() =>
        {
            Ok(())
        }
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}
