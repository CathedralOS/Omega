use optimization_core::OptimizationExecutionPhase;
use register_homes::{AllocationEvidence, RegisterHomeCustodyReceipt};
use selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator};
use selected_instructions_to_register_homes::AllocationOutput;

use super::model::OptimizedUnitFunctionRelativeRealizationError;

pub(super) fn validate_source(
    current: &AllocationOutput<'_>,
) -> Result<RegisterHomeCustodyReceipt, OptimizedUnitFunctionRelativeRealizationError> {
    let AllocationEvidence::RegisterHomes(source) = current.evidence() else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::RootMismatch);
    };
    let selections = current.selections();
    if [
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::AllocationRecovery,
        OptimizationExecutionPhase::PostAllocationMachine,
        OptimizationExecutionPhase::FunctionRelativeLayout,
    ]
    .into_iter()
    .any(|phase| !selections.for_phase(phase).is_empty())
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedSelectionPhase);
    }
    validate_unit_shape(current.selected_plan())?;
    Ok(*source)
}

pub fn validate_unit_shape(
    selected: &SelectedInstructionPlan,
) -> Result<(), OptimizedUnitFunctionRelativeRealizationError> {
    if selected.functions.is_empty()
        || !selected.structural_unit_functions.is_empty()
        || !selected.projected_structural_call_returns.is_empty()
        || !selected
            .functions
            .iter()
            .any(|function| function.machine == selected.entry)
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    }
    for function in &selected.functions {
        let [block] = function.blocks.as_slice() else {
            return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
        };
        let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
            return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
        };
        // Attachment is semantic identity, not a runtime receiver. Selection
        // has already retained every ABI input as an operand/register fact.
        if function.entry_block != block.id
            || !function.virtual_registers.is_empty()
            || !block.instructions.is_empty()
            || instruction.kind != SelectedInstructionKind::ReturnUnit
            || !instruction.operands.is_empty()
        {
            return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
        }
    }
    Ok(())
}
