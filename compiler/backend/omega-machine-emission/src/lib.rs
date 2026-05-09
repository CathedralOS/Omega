use omega_calling_conventions::HostAbiPlan;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_program::{MachineCodePlan, MachineInstruction};
use omega_target::NativeTarget;
use omega_target_program::InstructionPlan;

mod branch_distances;
mod encoding;
mod host_bindings;

use encoding::encode_machine_instruction;

#[derive(Debug)]
pub struct MachineEmissionInput<'plan, 'machine> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub machine_code: &'machine mut MachineCodePlan,
    pub host_abi: &'plan HostAbiPlan,
    pub terminal_dispatch_index: u32,
}

pub fn emit_machine_bytes(input: MachineEmissionInput<'_, '_>) -> Result<(), Diagnostic> {
    input.machine_code.bytes.clear();

    let function_spans = input
        .machine_code
        .functions
        .iter()
        .map(|(_, function)| function.instructions)
        .collect::<Vec<_>>();

    for instructions in function_spans {
        emit_function_bytes(
            input.target,
            input.instructions,
            input.host_abi,
            input.terminal_dispatch_index,
            input.machine_code,
            instructions,
        )?;
    }

    Ok(())
}

fn emit_function_bytes(
    target: NativeTarget,
    instructions: &InstructionPlan,
    host_abi: &HostAbiPlan,
    terminal_dispatch_index: u32,
    machine_code: &mut MachineCodePlan,
    machine_instructions_span: HandleSpan<MachineInstruction>,
) -> Result<(), Diagnostic> {
    let Some(machine_instructions) = machine_code.instructions.span(machine_instructions_span)
    else {
        return Ok(());
    };

    let machine_instructions = machine_instructions.to_vec();
    let emission_context = MachineEmissionContext {
        target,
        instructions,
        host_abi,
        terminal_dispatch_index,
    };

    for (machine_instruction_index, machine_instruction) in machine_instructions.iter().enumerate()
    {
        let selected_handle =
            Handle::from_arena_index(machine_instruction.selected_instruction_index);
        let selected_instruction = instructions.instructions.get(selected_handle);
        let encoded = encode_machine_instruction(
            emission_context,
            &machine_instructions,
            machine_instruction_index,
            &selected_instruction.kind,
        )?;
        let byte_span = machine_code.bytes.insert_many(encoded);
        let Some(output_instructions) = machine_code
            .instructions
            .span_mut(machine_instructions_span)
        else {
            continue;
        };
        output_instructions[machine_instruction_index].bytes = byte_span;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MachineEmissionContext<'plan> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub host_abi: &'plan HostAbiPlan,
    pub terminal_dispatch_index: u32,
}
