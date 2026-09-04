use super::super::error::*;
use super::super::prelude::*;

pub(in crate::stages::realization::function_relative_realization) fn rel8_selected(
    selections: &OptimizationSelections,
    architecture: omega_target::Architecture,
) -> Result<bool, FunctionRelativeOptimizationRealizationError> {
    let phase = selections.project_phase(OptimizationExecutionPhase::FunctionRelativeLayout);
    crate::stages::layout::x86_branch_relaxation::x86_rel8_selected(&phase, architecture)
        .map_err(FunctionRelativeOptimizationRealizationError::RuleCatalog)
}

pub(in crate::stages::realization::function_relative_realization) fn stage_selected_relaxation<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    selections: &OptimizationSelections,
    budget: OptimizationWorkBudget,
) -> Result<Option<StagedOptimizedX86BranchRelaxation>, FunctionRelativeOptimizationRealizationError>
{
    if !rel8_selected(selections, baseline_layout.target().architecture)? {
        return Ok(None);
    }
    stage_optimized_x86_branch_relaxation(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        budget,
    )
    .map(Some)
    .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation)
}

pub(in crate::stages::realization::function_relative_realization) fn validate_selected_relaxation<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match (
        rel8_selected(selections, baseline_layout.target().architecture)?,
        relaxation,
    ) {
        (false, None) => Ok(()),
        (true, Some(relaxation)) => validate_optimized_x86_branch_relaxation(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            relaxation,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::X86BranchRelaxation),
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}

pub(in crate::stages::realization::function_relative_realization) fn stage_exit_contract<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
) -> Result<ValidatedWholeFunctionExitContract, FunctionRelativeOptimizationRealizationError> {
    match relaxation {
        Some(relaxation) => stage_whole_function_exit_contract_after_x86_branch_relaxation(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            relaxation,
        ),
        None => stage_whole_function_exit_contract(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
        ),
    }
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

pub(in crate::stages::realization::function_relative_realization) fn validate_exit_contract<
    S: ValidatedSelectedAnalysis,
>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &omega_register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedWholeFunctionExitContract,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match relaxation {
        Some(relaxation) => validate_whole_function_exit_contract_after_x86_branch_relaxation(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            relaxation,
            exit_contract,
        ),
        None => validate_whole_function_exit_contract(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            exit_contract,
        ),
    }
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}

pub(in crate::stages::realization::function_relative_realization) fn final_layout<'layout>(
    baseline_layout: &'layout StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&'layout StagedOptimizedX86BranchRelaxation>,
) -> &'layout StagedOptimizedResolvedSelectedFormLayout {
    relaxation
        .map(StagedOptimizedX86BranchRelaxation::layout)
        .unwrap_or(baseline_layout)
}

pub(in crate::stages::realization::function_relative_realization) fn validate_relaxation_manifest_roots(
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    match (
        rel8_selected(selections, baseline_layout.target().architecture)?,
        relaxation,
    ) {
        (false, None) => Ok(()),
        (true, Some(relaxation))
            if relaxation.source() == baseline_layout.identity()
                && relaxation.output() == relaxation.layout().identity() =>
        {
            Ok(())
        }
        _ => Err(FunctionRelativeOptimizationRealizationError::RootMismatch),
    }
}
