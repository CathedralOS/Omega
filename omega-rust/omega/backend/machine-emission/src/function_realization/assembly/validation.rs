use super::super::{error::*, prelude::*};
use super::{validate_exit_contract, validate_selected_relaxation};

/// Independently replay every artifact produced by [`super::build_realization`].
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn validate_realization_artifacts<S: ValidatedSelectedAnalysis>(
    selected: &S,
    allocation: &selected_instructions_to_register_homes::AllocationOutput<'_>,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    physical: &register_model::ValidatedPhysicalRegisterModel,
    encoding: &StagedOptimizedSelectedFormEncoding,
    baseline_layout: &StagedOptimizedResolvedSelectedFormLayout,
    relaxation: Option<&StagedOptimizedX86BranchRelaxation>,
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
    validate_selected_relaxation(
        selected,
        machine,
        physical,
        encoding,
        baseline_layout,
        relaxation,
        selections,
    )?;
    super::super::unit::frame::validate_unit_frame(allocation, machine, frame)?;
    match frame {
        Some(frame) => validate_whole_function_exit_contract_with_frame(
            selected,
            machine,
            physical,
            encoding,
            super::final_layout(baseline_layout, relaxation),
            frame.layout(),
            frame.protocol(),
            exit_contract,
        )
        .map_err(FunctionRelativeOptimizationRealizationError::ExitContract),
        None => validate_exit_contract(
            selected,
            machine,
            physical,
            encoding,
            baseline_layout,
            relaxation,
            exit_contract,
        ),
    }
}
