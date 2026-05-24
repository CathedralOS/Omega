use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::{
    MachineInstruction, MachineInstructionFunction, MachineInstructionPlan,
};

mod shapes;

use shapes::lower_machine_instruction_kind;

pub fn build_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionPlan, Diagnostic> {
    let mut machine_instructions = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.functions.len(),
        assigned_target_operations.instructions.len(),
    );

    for (_, function) in assigned_target_operations.functions.iter() {
        let function_instructions = append_machine_instructions(
            assigned_target_operations,
            function,
            &mut machine_instructions.instructions,
        )?;

        machine_instructions
            .functions
            .insert(MachineInstructionFunction {
                source_key: function.source_key,
                instructions: function_instructions,
            });
    }

    Ok(machine_instructions)
}

fn append_machine_instructions(
    assigned_target_operations: &AssignedTargetOperationPlan,
    function: &omega_assigned_target_operations::AssignedTargetOperationFunction,
    output_instructions: &mut Arena<MachineInstruction>,
) -> Result<HandleSpan<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = assigned_target_operations
        .instructions
        .span(function.instructions)
    else {
        return Ok(HandleSpan::empty());
    };

    output_instructions.try_insert_many(selected_instructions.iter().enumerate().map(
        |(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");

            Ok(MachineInstruction {
                selected_instruction_index,
                kind: lower_machine_instruction_kind(&selected_instruction.kind)?,
            })
        },
    ))
}
