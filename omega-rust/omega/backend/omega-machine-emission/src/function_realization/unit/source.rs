use omega_optimization_core::OptimizationExecutionPhase;
use omega_selected_instructions::{
    SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator,
};
use omega_selected_instructions_to_register_homes::{AllocationEvidence, AllocationOutput};

use omega_selected_instructions_to_register_homes::StagedOptimizedRegisterHomeCustodyReceipt;

use super::model::OptimizedUnitFunctionRelativeRealizationError;

pub(super) fn validate_source(
    current: &AllocationOutput<'_>,
) -> Result<StagedOptimizedRegisterHomeCustodyReceipt, OptimizedUnitFunctionRelativeRealizationError>
{
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
    let [function] = selected.functions.as_slice() else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    };
    let [block] = function.blocks.as_slice() else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    };
    let SelectedTerminator::Return { instruction, .. } = &block.terminator else {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    };
    if selected.entry != function.machine
        || function.attachment.is_some()
        || function.entry_block != block.id
        || !function.virtual_registers.is_empty()
        || !block.instructions.is_empty()
        || instruction.kind != SelectedInstructionKind::ReturnUnit
        || !instruction.operands.is_empty()
    {
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedUnitShape);
    }
    Ok(())
}
