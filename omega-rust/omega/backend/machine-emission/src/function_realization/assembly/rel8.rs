use super::super::error::*;
use super::super::prelude::*;

pub(in crate::function_realization) fn rel8_selected(
    selections: &OptimizationSelections,
    architecture: target::Architecture,
) -> Result<bool, FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    resolved_layout_to_resolved_layout::x86_rel8_selected(&phase, architecture)
        .map_err(FunctionRelativeOptimizationRealizationError::RuleCatalog)
}

pub(in crate::function_realization) fn stage_layout_optimization<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<ResolvedLayoutOptimization, FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    execute_resolved_layout_optimization(
        selected,
        machine,
        physical,
        encoding,
        None,
        baseline_layout,
        &phase,
        budget,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)
}

pub(in crate::function_realization) fn validate_layout_optimization<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout: &ResolvedLayoutOptimization,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    validate_resolved_layout_optimization(
        selected,
        machine,
        physical,
        encoding,
        None,
        baseline_layout,
        &phase,
        layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::LayoutOptimization)
}

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
