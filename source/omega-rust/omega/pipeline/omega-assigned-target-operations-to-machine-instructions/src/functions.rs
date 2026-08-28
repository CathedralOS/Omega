use crate::shapes::lower_machine_instruction_kind;
use omega_assigned_target_operations::{
    AssignedTargetOperationFunction, AssignedTargetOperationPlan,
};
use omega_machine_instructions::MachineInstruction;
use psi_arena::{Arena, HandleSpan};
use psi_diagnostics::Diagnostic;

pub(crate) fn append_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
    function: &AssignedTargetOperationFunction,
    output_instructions: &mut Arena<MachineInstruction>,
) -> Result<HandleSpan<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = assigned_target_operations
        .code
        .instructions
        .span(function.instructions)
    else {
        return Ok(HandleSpan::empty());
    };
    crate::outgoing_stack_frames::validate_outgoing_stack_frames(
        assigned_target_operations.target,
        selected_instructions,
    )?;

    output_instructions.try_insert_many(selected_instructions.iter().enumerate().map(
        |(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");
            let selected_instruction_handle =
                psi_arena::Handle::from_arena_index(selected_instruction_index);

            Ok(MachineInstruction {
                selected_instruction_index,
                source_kind: selected_instruction.kind.clone(),
                kind: lower_machine_instruction_kind(
                    assigned_target_operations,
                    selected_instruction_handle,
                    &selected_instruction.kind,
                )?,
            })
        },
    ))
}
