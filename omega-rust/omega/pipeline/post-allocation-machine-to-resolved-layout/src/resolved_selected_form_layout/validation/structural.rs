use selected_instructions_to_register_homes::ValidatedSelectedAnalysis;

use crate::selected_form_encoding::{
    SelectedFormEncodingState, StagedOptimizedSelectedFormEncoding,
};
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;

use super::super::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
};

pub(super) fn validate<S: ValidatedSelectedAnalysis>(
    selected: &S,
    machine: &StagedOptimizedPostAllocationMachinePlan,
    pre_layout: &StagedOptimizedSelectedFormEncoding,
    artifact: &StagedOptimizedResolvedSelectedFormLayout,
) -> Result<(), OptimizedResolvedSelectedFormLayoutError> {
    let selected_plan = selected.selected_plan();
    let machine_plan = machine.machine().plan();
    if artifact.structural_unit_functions().len() != selected_plan.structural_unit_functions.len() {
        return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
    }
    for (((selected_function, machine_function), pre_function), candidate) in selected_plan
        .structural_unit_functions
        .iter()
        .zip(&machine_plan.structural_unit_functions)
        .zip(pre_layout.structural_unit_functions())
        .zip(artifact.structural_unit_functions())
    {
        if selected_function.machine != machine_function.machine
            || selected_function.machine != pre_function.machine
            || selected_function.entry_block != machine_function.block
            || selected_function.entry_block != pre_function.block
        {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralFunctionRosterMismatch(
                    selected_function.machine,
                ),
            );
        }
        if candidate.machine != selected_function.machine
            || candidate.block != selected_function.entry_block
            || candidate.offset != 0
        {
            return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
        }
        match (
            &selected_function.call,
            &machine_function.call,
            &pre_function.call,
            &candidate.call,
        ) {
            (None, None, None, None) => {}
            (Some(selected_call), Some(machine_call), Some(pre_call), Some(candidate_call)) => {
                if selected_call.id != machine_call.instruction
                    || selected_call.id != pre_call.instruction
                    || selected_call.operation != machine_call.operation
                    || selected_call.operation != pre_call.operation
                    || selected_call.callee != machine_call.callee
                    || selected_call.callee != pre_call.callee
                {
                    return Err(
                        OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                            selected_call.id,
                        ),
                    );
                }
                if candidate_call.instruction != pre_call.instruction
                    || candidate_call.operation != pre_call.operation
                    || candidate_call.callee != pre_call.callee
                    || candidate_call.offset != 0
                    || candidate_call.bytes != pre_call.bytes
                    || candidate_call.footprint != pre_call.footprint
                    || candidate_call.fixup != pre_call.fixup
                {
                    return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
                }
            }
            (Some(call), _, _, _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(call.id),
                );
            }
            (_, Some(call), _, _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        call.instruction,
                    ),
                );
            }
            (_, _, Some(call), _) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        call.instruction,
                    ),
                );
            }
            (_, _, _, Some(call)) => {
                return Err(
                    OptimizedResolvedSelectedFormLayoutError::StructuralCallRosterMismatch(
                        call.instruction,
                    ),
                );
            }
        }
        let selected_return = &selected_function.terminator.instruction;
        if selected_return.id != machine_function.return_instruction.instruction
            || selected_return.id != pre_function.return_instruction.instruction
            || machine_function.return_instruction.alternative.key
                != pre_function.return_instruction.alternative
        {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                    selected_return.id,
                ),
            );
        }
        let SelectedFormEncodingState::Encoded {
            bytes: return_bytes,
            ..
        } = &pre_function.return_instruction.state
        else {
            return Err(
                OptimizedResolvedSelectedFormLayoutError::StructuralReturnRosterMismatch(
                    selected_return.id,
                ),
            );
        };
        let return_offset = pre_function
            .call
            .as_ref()
            .map(|call| u64::try_from(call.bytes.len()))
            .transpose()
            .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?
            .unwrap_or(0);
        let byte_count = return_offset
            .checked_add(
                u64::try_from(return_bytes.len())
                    .map_err(|_| OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?,
            )
            .ok_or(OptimizedResolvedSelectedFormLayoutError::OffsetOverflow)?;
        if candidate.byte_count != byte_count
            || candidate.return_instruction.instruction != selected_return.id
            || candidate.return_instruction.alternative
                != pre_function.return_instruction.alternative
            || candidate.return_instruction.offset != return_offset
            || candidate.return_instruction.bytes != *return_bytes
            || candidate.return_instruction.branch.is_some()
        {
            return Err(OptimizedResolvedSelectedFormLayoutError::ArtifactMismatch);
        }
    }
    Ok(())
}
