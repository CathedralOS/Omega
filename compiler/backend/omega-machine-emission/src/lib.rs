use omega_calling_conventions::HostAbiPlan;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_bytes::{EncodedMachineFunction, EncodedMachineInstruction, EncodedMachinePlan};
use omega_machine_program::{MachineInstruction, MachineProgram};
use omega_target::NativeTarget;
use omega_target_operations::InstructionPlan;

mod branch_distances;
mod encoding;
mod host_bindings;
mod layout;

use encoding::encode_machine_instruction;
use layout::layout_machine_instructions;

#[derive(Debug)]
pub struct MachineEmissionInput<'plan, 'machine> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub machine_program: &'machine MachineProgram,
    pub host_abi: &'plan HostAbiPlan,
    pub terminal_dispatch_index: u32,
}

pub fn emit_machine_bytes(
    input: MachineEmissionInput<'_, '_>,
) -> Result<EncodedMachinePlan, Diagnostic> {
    let mut encoded_bytes = EncodedMachinePlan {
        target: input.target,
        functions: Arena::with_capacity(input.machine_program.functions.len()),
        instructions: Arena::with_capacity(input.machine_program.instructions.len()),
        bytes: Arena::new(),
        byte_count: 0,
    };

    for (_, function) in input.machine_program.functions.iter() {
        let byte_offset = encoded_bytes.bytes.len();
        emit_function_bytes(
            MachineEmissionContext {
                target: input.target,
                instructions: input.instructions,
                host_abi: input.host_abi,
                terminal_dispatch_index: input.terminal_dispatch_index,
            },
            input.machine_program,
            &mut encoded_bytes,
            function.instructions,
        )?;
        let byte_count = encoded_bytes.bytes.len() - byte_offset;
        encoded_bytes.functions.insert(EncodedMachineFunction {
            source_key: function.source_key,
            byte_offset,
            byte_count,
        });
    }

    encoded_bytes.byte_count = encoded_bytes.bytes.len();

    Ok(encoded_bytes)
}

fn emit_function_bytes(
    emission_context: MachineEmissionContext<'_>,
    machine_program: &MachineProgram,
    encoded_plan: &mut EncodedMachinePlan,
    machine_instructions_span: HandleSpan<MachineInstruction>,
) -> Result<(), Diagnostic> {
    let Some(machine_instructions) = machine_program.instructions.span(machine_instructions_span)
    else {
        return Ok(());
    };
    let laid_out_instructions =
        layout_machine_instructions(emission_context, machine_instructions)?;
    encoded_plan.bytes.reserve(
        laid_out_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum(),
    );

    for (machine_instruction_index, machine_instruction) in machine_instructions.iter().enumerate()
    {
        let selected_handle =
            Handle::from_arena_index(machine_instruction.selected_instruction_index);
        let selected_instruction = emission_context
            .instructions
            .instructions
            .get(selected_handle);
        let encoded = encode_machine_instruction(
            emission_context,
            &laid_out_instructions,
            machine_instruction_index,
            &selected_instruction.kind,
        )?;
        let byte_span = encoded_plan.bytes.insert_many(encoded);
        encoded_plan.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            bytes: byte_span,
        });
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
