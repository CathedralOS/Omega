use omega_calling_conventions::HostAbiPlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_target::NativeTarget;
use omega_target_program::InstructionPlan;

mod host_bindings;
mod shapes;

use omega_machine_program::{MachineCodePlan, MachineFunctionCode, MachineInstruction};
use shapes::machine_instruction_shape;

#[derive(Debug, Clone, Copy)]
pub struct TargetToMachineInput<'plan> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub host_abi: &'plan HostAbiPlan,
    pub terminal_dispatch_index: u32,
}

pub fn build_machine_code_plan(
    input: TargetToMachineInput<'_>,
) -> Result<MachineCodePlan, Diagnostic> {
    let mut machine_code_plan = MachineCodePlan {
        target: input.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        bytes: Arena::new(),
        byte_count: 0,
    };

    for (function_handle, function) in input.instructions.functions.iter() {
        let function_offset = machine_code_plan.byte_count;
        let machine_instructions = select_machine_instructions(input, function_offset, function)?;
        let function_byte_count = machine_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum();
        let instructions = machine_code_plan
            .instructions
            .insert_many(machine_instructions);

        machine_code_plan.functions.insert(MachineFunctionCode {
            source_function: function_handle,
            offset: function_offset,
            byte_count: function_byte_count,
            instructions,
        });
        machine_code_plan.byte_count += function_byte_count;
    }

    Ok(machine_code_plan)
}

fn select_machine_instructions(
    input: TargetToMachineInput<'_>,
    function_offset: usize,
    function: &omega_target_program::FunctionInstructionPlan,
) -> Result<Vec<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = input.instructions.instructions.span(function.instructions)
    else {
        return Ok(Vec::new());
    };

    let mut offset = function_offset;
    let machine_instructions = selected_instructions
        .iter()
        .enumerate()
        .map(|(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");
            let (kind, byte_width) = machine_instruction_shape(input, &selected_instruction.kind);
            let instruction = MachineInstruction {
                selected_instruction_index,
                offset,
                byte_width,
                bytes: HandleSpan::empty(),
                kind,
            };
            offset += byte_width;
            instruction
        })
        .collect::<Vec<_>>();

    Ok(machine_instructions)
}
