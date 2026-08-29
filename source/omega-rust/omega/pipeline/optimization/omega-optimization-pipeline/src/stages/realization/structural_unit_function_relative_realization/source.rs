use omega_optimization_core::OptimizationExecutionPhase;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan};
use omega_target::{Architecture, ObjectFormat};

use crate::{
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomes,
    StagedOptimizedSelectedInstructions, validate_optimized_register_home_custody,
};

use super::model::OptimizedStructuralUnitFunctionRelativeRealizationError;

pub(super) fn selected_stage(
    homes: &StagedOptimizedRegisterHomes,
) -> &StagedOptimizedSelectedInstructions {
    homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage()
}

pub(super) fn validate_source(
    homes: &StagedOptimizedRegisterHomes,
) -> Result<
    StagedOptimizedRegisterHomeCustodyReceipt,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let source = validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(OptimizedStructuralUnitFunctionRelativeRealizationError::Homes)?;
    if source != homes.custody() {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::ReceiptMismatch);
    }
    let selected_stage = selected_stage(homes);
    let selections = selected_stage.optimized_target().optimized().selections();
    if [
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::AllocationRecovery,
        OptimizationExecutionPhase::PostAllocationMachine,
        OptimizationExecutionPhase::FunctionRelativeLayout,
    ]
    .into_iter()
    .any(|phase| !selections.for_phase(phase).is_empty())
    {
        return Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedSelectionPhase,
        );
    }
    validate_structural_unit_shape(selected_stage.selected().selected_plan())?;
    Ok(source)
}

fn validate_structural_unit_shape(
    selected: &SelectedInstructionPlan,
) -> Result<(), OptimizedStructuralUnitFunctionRelativeRealizationError> {
    if selected.target.architecture != Architecture::X86_64
        || selected.target.object_format != ObjectFormat::Coff
        || !selected.functions.is_empty()
        || selected.structural_unit_functions.is_empty()
        || !selected
            .structural_unit_functions
            .iter()
            .any(|function| function.machine == selected.entry)
    {
        return Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedStructuralUnitShape,
        );
    }
    for function in &selected.structural_unit_functions {
        if function.terminator.instruction.kind != SelectedInstructionKind::ReturnUnit
            || !function.terminator.instruction.operands.is_empty()
            || function.call.as_ref().is_some_and(|call| {
                call.id == function.terminator.instruction.id
                    || !selected
                        .structural_unit_functions
                        .iter()
                        .any(|callee| callee.machine == call.callee)
            })
        {
            return Err(
                OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedStructuralUnitShape,
            );
        }
    }
    Ok(())
}
