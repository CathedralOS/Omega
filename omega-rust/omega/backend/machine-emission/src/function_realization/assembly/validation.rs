use super::super::{error::*, prelude::*};
use super::{validate_exit_contract, validate_selected_relaxation};

/// Independently replay every artifact produced by [`super::build_realization`].
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn validate_realization_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
    exit_contract: &ValidatedWholeFunctionExitContract,
    selections: &OptimizationSelections,
) -> Result<(), FunctionRelativeOptimizationRealizationError> {
    validate_optimized_layout_independent_selected_form_encoding(
        selected, machine, physical, encoding,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Encoding)?;
    validate_optimized_resolved_selected_form_layout(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::Layout)?;
    validate_selected_relaxation(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        selections,
    )?;
    validate_exit_contract(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        exit_contract,
    )
}
