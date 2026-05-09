use crate::plan::NativePlan;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;

mod branch_distances;
mod encoding;
mod host_bindings;
mod shapes;
mod widths;

use encoding::encode_machine_instruction;
use omega_machine_program::{MachineCodePlan, MachineFunctionCode, MachineInstruction};
use shapes::machine_instruction_shape;

pub fn build_machine_code_plan(native_plan: &NativePlan) -> Result<MachineCodePlan, Diagnostic> {
    let mut machine_code_plan = MachineCodePlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        bytes: Arena::new(),
        byte_count: 0,
    };

    for (_, function) in native_plan.instructions.functions.iter() {
        let function_offset = machine_code_plan.byte_count;
        let machine_instructions = select_machine_instructions(
            native_plan,
            function_offset,
            function,
            &mut machine_code_plan.bytes,
        )?;
        let function_byte_count = machine_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum();
        let instructions = machine_code_plan
            .instructions
            .insert_many(machine_instructions);

        machine_code_plan.functions.insert(MachineFunctionCode {
            symbol: function.symbol.clone(),
            offset: function_offset,
            byte_count: function_byte_count,
            instructions,
        });
        machine_code_plan.byte_count += function_byte_count;
    }

    Ok(machine_code_plan)
}

fn select_machine_instructions(
    native_plan: &NativePlan,
    function_offset: usize,
    function: &omega_target_program::FunctionInstructionPlan,
    bytes: &mut Arena<u8>,
) -> Result<Vec<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = native_plan
        .instructions
        .instructions
        .span(function.instructions)
    else {
        return Ok(Vec::new());
    };

    let mut offset = function_offset;
    let mut machine_instructions = selected_instructions
        .iter()
        .enumerate()
        .map(|(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");
            let (kind, byte_width) =
                machine_instruction_shape(native_plan, &selected_instruction.kind);
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

    for (selected_offset, selected_instruction) in selected_instructions.iter().enumerate() {
        let selected_instruction_index =
            machine_instructions[selected_offset].selected_instruction_index;
        let byte_span = bytes.insert_many(encode_machine_instruction(
            native_plan,
            &machine_instructions,
            selected_offset,
            &selected_instruction.kind,
        )?);

        debug_assert_eq!(
            selected_instruction_index,
            machine_instructions[selected_offset].selected_instruction_index
        );
        machine_instructions[selected_offset].bytes = byte_span;
    }

    Ok(machine_instructions)
}
