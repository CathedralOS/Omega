use super::backend_state_name;

use crate::BackendReportInput;
use omega_machine_program::{MachineFunction, MachineInstruction};
use omega_object::storage_region_symbol_name;
use omega_target_operations::{
    FunctionInstructionPlan, InstructionOperand, InstructionOperandKind, SelectedInstruction,
    SelectedInstructionKind, TargetDataObject,
};

pub(super) fn write_codegen_sections(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    output.push_str("## Native Data\n");
    output.push_str(&format!("objects: {}\n", backend_plan.data.objects.len()));
    output.push_str(&format!("bytes: {}\n", backend_plan.data.bytes.len()));
    if backend_plan.data.objects.is_empty() {
        output.push_str("none\n");
    } else {
        for (_, data_object) in backend_plan.data.objects.iter() {
            write_target_data_object(output, backend_plan, data_object);
        }
    }
    output.push('\n');

    output.push_str("## Instruction Selection\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.instructions.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.instructions.instructions.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        backend_plan.instructions.operands.len()
    ));
    for (_, function) in backend_plan.instructions.functions.iter() {
        write_function_instruction_plan(output, backend_plan, function);
    }
    output.push('\n');

    output.push_str("## Machine Program\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.machine_program.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.machine_program.instructions.len()
    ));
    output.push_str(&format!(
        "encoded bytes: {}\n",
        backend_plan.encoded_machine.bytes.len()
    ));
    output.push_str(&format!(
        "bytes: {}\n",
        backend_plan.encoded_machine.byte_count
    ));
    for (_, function) in backend_plan.machine_program.functions.iter() {
        write_machine_function_code(output, backend_plan, function);
    }
    output.push('\n');
}

fn write_target_data_object(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    data_object: &TargetDataObject,
) {
    let byte_count = backend_plan
        .data
        .bytes
        .span(data_object.bytes)
        .map_or(0, |bytes| bytes.len());
    let source_name = backend_state_name(backend_plan, data_object.source_key);

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
    backend_plan: &BackendReportInput<'_>,
    function: &FunctionInstructionPlan,
) {
    let source_name = backend_state_name(backend_plan, function.source_key);
    output.push_str(&format!(
        "- function {} from {}\n",
        function.symbol, source_name
    ));

    match backend_plan
        .instructions
        .instructions
        .span(function.instructions)
    {
        Some(instructions) if instructions.is_empty() => output.push_str("  instructions: none\n"),
        Some(instructions) => {
            output.push_str("  instructions:\n");
            for instruction in instructions {
                write_selected_instruction(output, backend_plan, instruction);
            }
        }
        None => output.push_str("  instructions: invalid span\n"),
    }
}

fn write_selected_instruction(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    instruction: &SelectedInstruction,
) {
    output.push_str(&format!(
        "    - statement {}: {}\n",
        instruction.source_statement,
        selected_instruction_name(backend_plan, instruction)
    ));
}

fn selected_instruction_name(
    backend_plan: &BackendReportInput<'_>,
    instruction: &SelectedInstruction,
) -> String {
    let kind = &instruction.kind;
    match kind {
        SelectedInstructionKind::EnterFunction => "enter function".to_owned(),
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            terminal_dispatch_index,
        } => {
            let current_state_slot = &backend_plan.runtime_dispatch_loop.current_state_slot;
            let next_state_slot = &backend_plan.runtime_dispatch_loop.next_state_slot;
            format!(
                "enter dispatch loop entry #{entry_dispatch_index} terminal #{terminal_dispatch_index} current `{current_state_slot}` next `{next_state_slot}`"
            )
        }
        SelectedInstructionKind::EnterDispatchCase { dispatch_index } => {
            let label = backend_plan
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
            storage_region,
            byte_offset,
            byte_size,
            expected_value,
            has_storage,
        } => {
            if *has_storage {
                format!(
                    "evaluate dispatch guard {guard_lowering:?}/{operator:?} {storage_region:?} offset {byte_offset} bytes {byte_size} expected {expected_value}"
                )
            } else {
                format!("evaluate dispatch guard {guard_lowering:?}/{operator:?}")
            }
        }
        SelectedInstructionKind::CompareRuntimeTextLiteral { buffer, literal } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            format!("compare runtime text `{buffer_symbol}` with {literal:?}")
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            source_offset,
            operator,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
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
                storage_region_symbol_name(*left_region, backend_plan.entry_machine_name());
            let right_symbol =
                storage_region_symbol_name(*right_region, backend_plan.entry_machine_name());
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
            let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
            format!(
                "compare runtime storage {symbol}@{byte_offset} {operator:?} {expected_value} bytes {byte_size}"
            )
        }
        SelectedInstructionKind::CompareRuntimeValues {
            left,
            right,
            byte_size,
            operator,
        } => {
            format!(
                "compare runtime values {} {operator:?} {} bytes {byte_size}",
                runtime_value_operand_name(left, backend_plan.entry_machine_name()),
                runtime_value_operand_name(right, backend_plan.entry_machine_name()),
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, literal } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            format!("write runtime text `{buffer_symbol}` = {literal:?}")
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            buffer,
            byte_offset,
            literal,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "append runtime text suffix {source_symbol}@{source_offset} -> `{buffer_symbol}`@{buffer_offset}, descriptor {target_symbol}@{target_offset}, len +{length_delta}"
            )
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer {
            buffer,
            target_region,
            target_offset,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "materialize runtime text buffer `{buffer_symbol}` for {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            buffer,
            pointer_byte_offset,
            field_byte_offset,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            format!(
                "materialize runtime text buffer `{buffer_symbol}` for runtime_frame@{pointer_byte_offset} +{field_byte_offset}"
            )
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            buffer,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            format!(
                "materialize runtime text buffer `{buffer_symbol}` for runtime_frame[{descriptor_offset}; index @{index_offset}; elem {element_byte_size}] +{field_byte_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            buffer,
            source_region,
            source_offset,
            target_region,
            target_offset,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "append runtime text stored place {source_symbol}@{source_offset} -> `{buffer_symbol}`, descriptor {target_symbol}@{target_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            buffer,
            source_region,
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            format!(
                "append runtime text stored place {source_symbol}@{source_offset} -> `{buffer_symbol}`, descriptor runtime_frame@{pointer_byte_offset} +{field_byte_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            buffer,
            source_region,
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            format!(
                "append runtime text stored place {source_symbol}@{source_offset} -> `{buffer_symbol}`, descriptor runtime_frame[{descriptor_offset}; index @{index_offset}; elem {element_byte_size}] +{field_byte_offset}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            buffer,
            target_region,
            target_offset,
            literal,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "append runtime text literal `{buffer_symbol}`, descriptor {target_symbol}@{target_offset} += {literal:?}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            buffer,
            pointer_byte_offset,
            field_byte_offset,
            literal,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            format!(
                "append runtime text literal `{buffer_symbol}`, descriptor runtime_frame@{pointer_byte_offset} +{field_byte_offset} += {literal:?}"
            )
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            buffer,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            literal,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            format!(
                "append runtime text literal `{buffer_symbol}`, descriptor runtime_frame[{descriptor_offset}; index @{index_offset}; elem {element_byte_size}] +{field_byte_offset} += {literal:?}"
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
        SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region,
            byte_offset,
            byte_size,
            value,
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "write runtime storage integer {target_symbol}@{byte_offset} bytes {byte_size} value {value}"
            )
        }
        SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            value,
        } => format!(
            "write runtime pointee integer runtime_frame@{pointer_byte_offset} +{field_byte_offset} bytes {byte_size} value {value}"
        ),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_region,
            target_offset,
            byte_size,
            left,
            operator,
            right,
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "write runtime storage binary {target_symbol}@{target_offset} bytes {byte_size} {} {operator:?} {}",
                runtime_value_operand_name(left, backend_plan.entry_machine_name()),
                runtime_value_operand_name(right, backend_plan.entry_machine_name()),
            )
        }
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => {
            format!(
                "write runtime pointee binary runtime_frame@{pointer_byte_offset} +{field_byte_offset} bytes {byte_size} {} {operator:?} {}",
                runtime_value_operand_name(left, backend_plan.entry_machine_name()),
                runtime_value_operand_name(right, backend_plan.entry_machine_name()),
            )
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => {
            format!(
                "write runtime-frame indexed integer descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} bytes {byte_size} value {value}"
            )
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => {
            format!(
                "write runtime-frame indexed binary descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} bytes {byte_size} {} {operator:?} {}",
                runtime_value_operand_name(left, backend_plan.entry_machine_name()),
                runtime_value_operand_name(right, backend_plan.entry_machine_name()),
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_str();
            format!(
                "write runtime machine string offset {byte_offset} data `{data_symbol}` len {byte_length}"
            )
        }
        SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset,
            field_byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_str();
            format!(
                "write runtime pointee string runtime_frame@{pointer_byte_offset} +{field_byte_offset} data `{data_symbol}` len {byte_length}"
            )
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            target_offset,
            byte_capacity,
            source,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_str();
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "read runtime text line {} -> `{buffer_symbol}` cap {byte_capacity}, descriptor {target_symbol}@{target_offset}",
                runtime_text_read_source_name(source)
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
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "copy runtime storage {source_symbol}@{source_offset} -> {target_symbol}@{target_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
            source_region,
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            format!(
                "copy runtime storage {source_symbol}@{source_offset} -> runtime-frame indexed descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
            source_region,
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            format!(
                "copy runtime storage {source_symbol}@{source_offset} -> runtime pointee runtime_frame@{pointer_byte_offset} +{field_byte_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            format!("set dispatch state #{dispatch_index}")
        }
        SelectedInstructionKind::TerminateDispatch => "terminate dispatch".to_owned(),
        SelectedInstructionKind::LeaveDispatchCase => "leave dispatch case".to_owned(),
        SelectedInstructionKind::LeaveDispatchLoop => "leave dispatch loop".to_owned(),
        SelectedInstructionKind::BeginPlatformCall => {
            let platform_call = backend_plan
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
            operation_key,
            operands,
        } => {
            format!(
                "call host operation {}.{}({})",
                operation_key.capability_name(),
                operation_key.operation_name(),
                selected_instruction_operands_name(backend_plan, *operands)
            )
        }
        SelectedInstructionKind::LeaveFunction => "leave function".to_owned(),
    }
}

fn runtime_text_read_source_name(
    source: &omega_target_operations::RuntimeTextReadSource,
) -> String {
    match source {
        omega_target_operations::RuntimeTextReadSource::Import { symbol } => {
            format!("import {symbol}")
        }
        omega_target_operations::RuntimeTextReadSource::Syscall {
            number,
            number_register,
            supervisor_call,
        } => format!("syscall {number} via x{number_register}/svc #{supervisor_call}"),
    }
}

fn runtime_value_operand_name(
    operand: &omega_target_operations::RuntimeValueOperand,
    entry_machine_name: &str,
) -> String {
    match operand {
        omega_target_operations::RuntimeValueOperand::Immediate(value) => value.to_string(),
        omega_target_operations::RuntimeValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => {
            let symbol = storage_region_symbol_name(*region, entry_machine_name);
            format!("{symbol}@{byte_offset}/{}", byte_size)
        }
        omega_target_operations::RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => format!(
            "*frame@{pointer_byte_offset}+{field_byte_offset}/{}",
            byte_size
        ),
        omega_target_operations::RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame_indexed(descriptor@{descriptor_offset}, index@{index_offset}, elem {element_byte_size}, field +{field_byte_offset}, bytes {byte_size})"
        ),
        omega_target_operations::RuntimeValueOperand::Binary {
            left,
            operator,
            right,
        } => format!(
            "({} {operator:?} {})",
            runtime_value_operand_name(left, entry_machine_name),
            runtime_value_operand_name(right, entry_machine_name),
        ),
    }
}

fn selected_instruction_operands_name(
    backend_plan: &BackendReportInput<'_>,
    operands: omega_core::arena::HandleSpan<InstructionOperand>,
) -> String {
    let Some(operands) = backend_plan.instructions.operands.span(operands) else {
        return "invalid operands".to_owned();
    };

    operands
        .iter()
        .map(|operand| match &operand.kind {
            InstructionOperandKind::DataAddress { data } => {
                let symbol = backend_plan.data.objects.get(*data).symbol.as_str();
                format!("addr {symbol}")
            }
            InstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("string ptr {symbol}@{byte_offset}")
            }
            InstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => {
                let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
                format!("string len {symbol}@{byte_offset}")
            }
            InstructionOperandKind::ImmediateInteger(value) => value.to_string(),
            InstructionOperandKind::ByteLength(value) => format!("len {value}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_machine_function_code(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    function: &MachineFunction,
) {
    let function_symbol = machine_function_symbol(backend_plan, function);
    output.push_str(&format!("- function {}\n", function_symbol));

    match backend_plan
        .machine_program
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
    _backend_plan: &BackendReportInput<'_>,
    function: &MachineFunction,
) -> String {
    function.symbol.clone()
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
    let Some((_, encoded_instruction)) =
        backend_plan
            .encoded_machine
            .instructions
            .iter()
            .find(|(_, encoded_instruction)| {
                encoded_instruction.selected_instruction_index
                    == instruction.selected_instruction_index
            })
    else {
        return "invalid".to_owned();
    };
    let Some(bytes) = backend_plan
        .encoded_machine
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
