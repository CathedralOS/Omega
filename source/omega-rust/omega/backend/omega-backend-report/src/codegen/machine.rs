use crate::BackendReportInput;
use omega_machine_instructions::{MachineInstruction, MachineInstructionFunction};

pub(super) fn write_machine_instructions_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Machine Instructions\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.machine_instructions.code.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.machine_instructions.code.instructions.len()
    ));
    output.push_str(&format!(
        "encoded bytes: {}\n",
        backend_plan.encoded_machine.code.bytes.len()
    ));
    output.push_str(&format!(
        "bytes: {}\n",
        backend_plan.encoded_machine.code.byte_count
    ));
    for (_, function) in backend_plan.machine_instructions.code.functions.iter() {
        write_machine_function_code(output, backend_plan, function);
    }
    output.push('\n');
}

fn write_machine_function_code(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    function: &MachineInstructionFunction,
) {
    let function_symbol = machine_function_symbol(backend_plan, function);
    output.push_str(&format!("- function {}\n", function_symbol));

    match backend_plan
        .machine_instructions
        .code
        .instructions
        .span(function.instructions)
    {
        Some(instructions) if instructions.is_empty() => output.push_str("  instructions: none\n"),
        Some(instructions) => {
            output.push_str("  instructions:\n");
            for instruction in instructions {
                write_machine_instruction(output, backend_plan, instruction);
            }
        }
        None => output.push_str("  instructions: invalid span\n"),
    }
}

fn machine_function_symbol(
    backend_plan: &BackendReportInput<'_>,
    function: &MachineInstructionFunction,
) -> String {
    backend_plan
        .target_operations
        .code
        .functions
        .iter()
        .find(|(_, instruction_function)| instruction_function.identity == function.identity)
        .map(|(_, instruction_function)| instruction_function.symbol.to_string())
        .unwrap_or_else(|| format!("{:?}", function.identity))
}

fn write_machine_instruction(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    instruction: &MachineInstruction,
) {
    output.push_str(&format!(
        "    - selected #{} {:?} encoded {}\n",
        instruction.selected_instruction_index,
        instruction.kind,
        machine_instruction_bytes_name(backend_plan, instruction)
    ));
}

fn machine_instruction_bytes_name(
    backend_plan: &BackendReportInput<'_>,
    instruction: &MachineInstruction,
) -> String {
    let Some((_, encoded_instruction)) = backend_plan
        .encoded_machine
        .code
        .instructions
        .iter()
        .find(|(_, encoded_instruction)| {
            encoded_instruction.selected_instruction_index == instruction.selected_instruction_index
        })
    else {
        return "invalid".to_owned();
    };
    let Some(bytes) = backend_plan
        .encoded_machine
        .code
        .bytes
        .span(encoded_instruction.bytes)
    else {
        return "invalid".to_owned();
    };

    if bytes.is_empty() {
        return "none".to_owned();
    }

    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
