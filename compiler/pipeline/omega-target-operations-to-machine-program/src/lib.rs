use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_program::{MachineFunction, MachineInstruction, MachineProgram};
use omega_target_operations::InstructionPlan;

mod shapes;

use shapes::lower_machine_instruction_kind;

pub fn build_machine_program(instructions: &InstructionPlan) -> Result<MachineProgram, Diagnostic> {
    let mut machine_program = MachineProgram {
        target: instructions.target,
        functions: Arena::with_capacity(instructions.functions.len()),
        instructions: Arena::with_capacity(instructions.instructions.len()),
    };

    for (_, function) in instructions.functions.iter() {
        let machine_instructions =
            append_machine_instructions(instructions, function, &mut machine_program.instructions)?;

        machine_program.functions.insert(MachineFunction {
            source_key: function.source_key,
            instructions: machine_instructions,
        });
    }

    Ok(machine_program)
}

fn append_machine_instructions(
    instructions: &InstructionPlan,
    function: &omega_target_operations::FunctionInstructionPlan,
    output_instructions: &mut Arena<MachineInstruction>,
) -> Result<HandleSpan<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = instructions.instructions.span(function.instructions) else {
        return Ok(HandleSpan::empty());
    };

    let mut start = Handle::invalid();
    let mut count = 0u32;

    for (selected_offset, selected_instruction) in selected_instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
            .expect("selected instruction overflow");
        let handle = output_instructions.append(MachineInstruction {
            selected_instruction_index,
            kind: lower_machine_instruction_kind(&selected_instruction.kind)?,
        });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("machine instruction span count overflow");
    }

    Ok(if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    })
}
