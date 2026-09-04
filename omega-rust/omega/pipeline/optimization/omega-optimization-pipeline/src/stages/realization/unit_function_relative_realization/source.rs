use omega_optimization_core::OptimizationExecutionPhase;
use omega_regalloc::ValidatedSelectedAnalysis;
use omega_selected_instructions::{
    SelectedInstructionKind, SelectedInstructionPlan, SelectedTerminator,
};

use crate::{
    StagedOptimizedRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomes,
    StagedOptimizedSelectedInstructions, validate_optimized_register_home_custody,
};

use super::model::OptimizedUnitFunctionRelativeRealizationError;

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
) -> Result<StagedOptimizedRegisterHomeCustodyReceipt, OptimizedUnitFunctionRelativeRealizationError>
{
    let source = validate_optimized_register_home_custody(
        homes.legality_stage(),
        homes.homes(),
        homes.post_allocation_manifest(),
    )
    .map_err(OptimizedUnitFunctionRelativeRealizationError::Homes)?;
    if source != homes.custody() {
        return Err(OptimizedUnitFunctionRelativeRealizationError::ReceiptMismatch);
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
        return Err(OptimizedUnitFunctionRelativeRealizationError::UnsupportedSelectionPhase);
    }
    validate_unit_shape(selected_stage.selected().selected_plan())?;
    Ok(source)
}

pub(crate) fn validate_unit_shape(
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
