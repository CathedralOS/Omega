use super::native_state_name;

use crate::plan::NativePlan;
use omega_machine_program::{MachineFunctionCode, MachineInstruction};
use omega_object::storage_region_symbol_name;
use omega_target_program::{
    FunctionInstructionPlan, InstructionOperand, InstructionOperandKind, NativeDataObject,
    SelectedInstruction, SelectedInstructionKind,
};

pub(super) fn write_codegen_sections(output: &mut String, native_plan: &NativePlan) {
    output.push_str("## Native Data\n");
    output.push_str(&format!("objects: {}\n", native_plan.data.objects.len()));
    output.push_str(&format!("bytes: {}\n", native_plan.data.bytes.len()));
    if native_plan.data.objects.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, data_object) in native_plan.data.objects.iter() {
            write_native_data_object(output, native_plan, data_object);
        }
    }
    output.push('\n');

    output.push_str("## Instruction Selection\n");
    output.push_str(&format!(
        "functions: {}\n",
        native_plan.instructions.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        native_plan.instructions.instructions.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        native_plan.instructions.operands.len()
    ));
    for (_, function) in native_plan.instructions.functions.iter() {
        write_function_instruction_plan(output, native_plan, function);
    }
    output.push('\n');

    output.push_str("## Machine Code Shape\n");
    output.push_str(&format!(
        "functions: {}\n",
        native_plan.machine_code.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        native_plan.machine_code.instructions.len()
    ));
    output.push_str(&format!(
        "encoded bytes: {}\n",
        native_plan.machine_code.bytes.len()
    ));
    output.push_str(&format!("bytes: {}\n", native_plan.machine_code.byte_count));
    for (_, function) in native_plan.machine_code.functions.iter() {
        write_machine_function_code(output, native_plan, function);
    }
    output.push('\n');
}

fn write_native_data_object(
    output: &mut String,
    native_plan: &NativePlan,
    data_object: &NativeDataObject,
) {
    let byte_count = native_plan
        .data
        .bytes
        .span(data_object.bytes)
        .map_or(0, |bytes| bytes.len());
    let source_name = native_state_name(native_plan, data_object.source_key);

    output.push_str(&format!(
        "- {} @{} bytes {} align {} from {} statement {}\n",
        data_object.symbol,
        data_object.offset,
        byte_count,
        data_object.alignment,
        source_name,
        data_object.source_statement
    ));
}

fn write_function_instruction_plan(
    output: &mut String,
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
) {
    let source_name = native_state_name(native_plan, function.source_key);
    output.push_str(&format!(
        "- function {} from {}\n",
        function.symbol, source_name
    ));

    match native_plan
        .instructions
        .instructions
        .span(function.instructions)
    {
        Some(instructions) if instructions.is_empty() => output.push_str("  instructions: none\n"),
        Some(instructions) => {
            output.push_str("  instructions:\n");
            for instruction in instructions {
                write_selected_instruction(output, native_plan, instruction);
            }
        }
        None => output.push_str("  instructions: invalid span\n"),
    }
}

fn write_selected_instruction(
    output: &mut String,
    native_plan: &NativePlan,
    instruction: &SelectedInstruction,
) {
    output.push_str(&format!(
        "    - statement {}: {}\n",
        instruction.source_statement,
        selected_instruction_name(native_plan, instruction)
    ));
}

fn selected_instruction_name(
    native_plan: &NativePlan,
    instruction: &SelectedInstruction,
) -> String {
    let kind = &instruction.kind;
    match kind {
        SelectedInstructionKind::EnterFunction => "enter function".to_owned(),
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            terminal_dispatch_index,
        } => {
            let current_state_slot = &native_plan.runtime_dispatch_loop.current_state_slot;
            let next_state_slot = &native_plan.runtime_dispatch_loop.next_state_slot;
            format!(
                "enter dispatch loop entry #{entry_dispatch_index} terminal #{terminal_dispatch_index} current `{current_state_slot}` next `{next_state_slot}`"
            )
        }
        SelectedInstructionKind::EnterDispatchCase { dispatch_index } => {
            let label = native_plan
                .runtime_dispatch_loop
                .cases
                .iter()
                .find(|(_, dispatch_case)| dispatch_case.dispatch_index == *dispatch_index)
                .map(|(_, dispatch_case)| dispatch_case.label.as_str())
                .unwrap_or("unknown");
            format!("enter dispatch case #{dispatch_index} `{label}`")
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering,
            operator,
            byte_offset,
            byte_size,
            expected_value,
            has_storage,
        } => {
            if *has_storage {
                format!(
                    "evaluate dispatch guard {guard_lowering:?}/{operator:?} offset {byte_offset} bytes {byte_size} expected {expected_value}"
                )
            } else {
                format!("evaluate dispatch guard {guard_lowering:?}/{operator:?}")
            }
        }
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            format!("compare runtime text `{buffer_symbol}` with {literal:?}")
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            source_offset,
            operator,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, native_plan.entry_machine_name());
            format!(
                "compare runtime text storage {source_symbol}@{source_offset} {operator:?} `{buffer_symbol}`"
            )
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_region,
            left_offset,
            right_region,
            right_offset,
            byte_size,
            operator,
        } => {
            let left_symbol =
                storage_region_symbol_name(*left_region, native_plan.entry_machine_name());
            let right_symbol =
                storage_region_symbol_name(*right_region, native_plan.entry_machine_name());
            format!(
                "compare runtime storage {left_symbol}@{left_offset} {operator:?} {right_symbol}@{right_offset} bytes {byte_size}"
            )
        }
        SelectedInstructionKind::CompareRuntimeStorageValue {
            region,
            byte_offset,
            byte_size,
            expected_value,
            operator,
        } => {
            let symbol = storage_region_symbol_name(*region, native_plan.entry_machine_name());
            format!(
                "compare runtime storage {symbol}@{byte_offset} {operator:?} {expected_value} bytes {byte_size}"
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, literal } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            format!("write runtime text `{buffer_symbol}` = {literal:?}")
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            buffer,
            byte_offset,
            literal,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            format!("write runtime text segment `{buffer_symbol}`@{byte_offset} = {literal:?}")
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            buffer_offset,
            source_region,
            source_offset,
            target_region,
            target_offset,
            length_delta,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, native_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, native_plan.entry_machine_name());
            format!(
                "append runtime text suffix {source_symbol}@{source_offset} -> `{buffer_symbol}`@{buffer_offset}, descriptor {target_symbol}@{target_offset}, len +{length_delta}"
            )
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer,
            target_region,
            target_offset,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            let target_symbol =
                storage_region_symbol_name(*target_region, native_plan.entry_machine_name());
            format!(
                "materialize runtime text buffer `{buffer_symbol}` for {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer,
            source_region,
            source_offset,
            target_region,
            target_offset,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, native_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, native_plan.entry_machine_name());
            format!(
                "append runtime text stored place {source_symbol}@{source_offset} -> `{buffer_symbol}`, descriptor {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer,
            target_region,
            target_offset,
            literal,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            let target_symbol =
                storage_region_symbol_name(*target_region, native_plan.entry_machine_name());
            format!(
                "append runtime text literal `{buffer_symbol}`, descriptor {target_symbol}@{target_offset} += {literal:?}"
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => {
            format!(
                "write runtime machine integer offset {byte_offset} bytes {byte_size} value {value}"
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = native_plan.data.objects.get(*data).symbol.as_str();
            format!(
                "write runtime machine string offset {byte_offset} data `{data_symbol}` len {byte_length}"
            )
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            target_offset,
            byte_capacity,
            syscall_number,
            syscall_number_register,
            supervisor_call,
        } => {
            let buffer_symbol = native_plan.data.objects.get(*buffer).symbol.as_str();
            let target_symbol =
                storage_region_symbol_name(*target_region, native_plan.entry_machine_name());
            format!(
                "read runtime text line syscall {syscall_number} via x{syscall_number_register}/svc #{supervisor_call} -> `{buffer_symbol}` cap {byte_capacity}, descriptor {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::CopyRuntimeStorage {
            source_region,
            source_offset,
            target_region,
            target_offset,
            byte_count,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, native_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, native_plan.entry_machine_name());
            format!(
                "copy runtime storage {source_symbol}@{source_offset} -> {target_symbol}@{target_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            format!("set dispatch state #{dispatch_index}")
        }
        SelectedInstructionKind::TerminateDispatch => "terminate dispatch".to_owned(),
        SelectedInstructionKind::LeaveDispatchCase => "leave dispatch case".to_owned(),
        SelectedInstructionKind::LeaveDispatchLoop => "leave dispatch loop".to_owned(),
        SelectedInstructionKind::BeginPlatformCall => {
            let platform_call = native_plan
                .host_calls
                .calls
                .iter()
                .find(|(_, call)| {
                    call.source_key == instruction.source_key
                        && call.statement_index == instruction.source_statement
                })
                .map(|(_, call)| call.platform_call.as_str())
                .unwrap_or("unknown");
            format!("begin platform call `{platform_call}`")
        }
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            format!(
                "call host operation {capability}.{operation}({})",
                selected_instruction_operands_name(native_plan, *operands)
            )
        }
        SelectedInstructionKind::LeaveFunction => "leave function".to_owned(),
    }
}

fn selected_instruction_operands_name(
    native_plan: &NativePlan,
    operands: omega_core::arena::HandleSpan<InstructionOperand>,
) -> String {
    let Some(operands) = native_plan.instructions.operands.span(operands) else {
        return "invalid operands".to_owned();
    };

    operands
        .iter()
        .map(|operand| match &operand.kind {
            InstructionOperandKind::DataAddress { data } => {
                let symbol = native_plan.data.objects.get(*data).symbol.as_str();
                format!("addr {symbol}")
            }
            InstructionOperandKind::RuntimeMachineStringPointer { byte_offset } => {
                format!("machine string ptr @{byte_offset}")
            }
            InstructionOperandKind::RuntimeMachineStringLength { byte_offset } => {
                format!("machine string len @{byte_offset}")
            }
            InstructionOperandKind::ImmediateInteger(value) => value.to_string(),
            InstructionOperandKind::ByteLength(value) => format!("len {value}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_machine_function_code(
    output: &mut String,
    native_plan: &NativePlan,
    function: &MachineFunctionCode,
) {
    let function_symbol = machine_function_symbol(native_plan, function);
    output.push_str(&format!(
        "- function {} @{} bytes {}\n",
        function_symbol, function.offset, function.byte_count
    ));

    match native_plan
        .machine_code
        .instructions
        .span(function.instructions)
    {
        Some(instructions) if instructions.is_empty() => output.push_str("  instructions: none\n"),
        Some(instructions) => {
            output.push_str("  instructions:\n");
            for instruction in instructions {
                write_machine_instruction(output, native_plan, instruction);
            }
        }
        None => output.push_str("  instructions: invalid span\n"),
    }
}

fn machine_function_symbol(native_plan: &NativePlan, function: &MachineFunctionCode) -> String {
    native_plan
        .instructions
        .functions
        .get(function.source_function)
        .symbol
        .clone()
}

fn write_machine_instruction(
    output: &mut String,
    native_plan: &NativePlan,
    instruction: &MachineInstruction,
) {
    output.push_str(&format!(
        "    - selected #{} @{} bytes {} {:?} encoded {}\n",
        instruction.selected_instruction_index,
        instruction.offset,
        instruction.byte_width,
        instruction.kind,
        machine_instruction_bytes_name(native_plan, instruction)
    ));
}

fn machine_instruction_bytes_name(
    native_plan: &NativePlan,
    instruction: &MachineInstruction,
) -> String {
    let Some(bytes) = native_plan.machine_code.bytes.span(instruction.bytes) else {
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
