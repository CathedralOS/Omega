use super::super::{error::*, prelude::*};
use super::validate_layout_optimization;

/// Independently replay every artifact produced by [`super::build_realization`].
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn validate_realization_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    allocation: &selected_instructions_to_register_homes::AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    layout_optimization: &ResolvedLayoutOptimization,
    frame: Option<&super::super::UnitSavedReturnAddressFrame>,
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
    validate_layout_optimization(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        layout_optimization,
        selections,
    )?;
    super::super::unit::frame::validate_unit_frame(allocation, machine, frame)?;
    crate::validate_whole_function_exit_contract_for_layout(
        selected,
        machine,
        physical,
        encoding,
        None,
        baseline_layout,
        layout_optimization,
        frame.map(|frame| (frame.layout(), frame.protocol())),
        exit_contract,
    )
    .map_err(FunctionRelativeOptimizationRealizationError::ExitContract)
}
