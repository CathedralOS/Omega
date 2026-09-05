use omega_optimization_core::OptimizationExecutionPhase;
use omega_selected_instructions::{SelectedInstructionKind, SelectedInstructionPlan};
use omega_selected_instructions_to_register_homes::{AllocationEvidence, AllocationOutput};
use omega_target::{Architecture, ObjectFormat};

use crate::StagedOptimizedRegisterHomeCustodyReceipt;

use super::model::OptimizedStructuralUnitFunctionRelativeRealizationError;

pub(super) fn validate_source(
    current: &AllocationOutput<'_>,
) -> Result<
    StagedOptimizedRegisterHomeCustodyReceipt,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
> {
    let AllocationEvidence::RegisterHomes(source) = current.evidence() else {
        return Err(OptimizedStructuralUnitFunctionRelativeRealizationError::RootMismatch);
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
        return Err(
            OptimizedStructuralUnitFunctionRelativeRealizationError::UnsupportedSelectionPhase,
        );
    }
    validate_structural_unit_shape(current.selected_plan())?;
    Ok(*source)
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
