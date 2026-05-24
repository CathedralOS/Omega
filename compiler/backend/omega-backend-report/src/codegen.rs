use super::backend_state_name;
use super::host::host_call_display_name;

use crate::BackendReportInput;
use omega_machine_instructions::{MachineInstruction, MachineInstructionFunction};
use omega_object_file::storage_region_symbol_name;
use omega_state_dispatch::state_dispatch_label;
use omega_target_operations::{
    InstructionOperand, InstructionOperandKind, RuntimeValueOperandHandle, TargetDataObject,
    SelectedInstructionKind, TargetOperation, TargetOperationFunction, TargetOperationKind,
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

    output.push_str("## Abstract Operations\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.abstract_operations.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.abstract_operations.instructions.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        backend_plan.abstract_operations.operands.len()
    ));
    output.push('\n');

    output.push_str("## Target Operations\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.target_operations.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.target_operations.instructions.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        backend_plan.target_operations.operands.len()
    ));
    for (_, function) in backend_plan.target_operations.functions.iter() {
        write_function_instruction_plan(output, backend_plan, function);
    }
    output.push('\n');

    output.push_str("## Assigned Target Operations\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.assigned_target_operations.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.assigned_target_operations.instructions.len()
    ));
    output.push_str(&format!(
        "runtime value homes: {}\n",
        backend_plan.assigned_target_operations.runtime_value_homes.len()
    ));
    let scratch_home_count = backend_plan
        .assigned_target_operations
        .runtime_value_homes
        .iter()
        .filter(|(_, home)| {
            matches!(
                home.kind,
                omega_assigned_target_operations::AssignedValueHomeKind::ScratchRegister { .. }
            )
        })
        .count();
    output.push_str(&format!("scratch homes: {}\n", scratch_home_count));
    write_assigned_value_homes(output, backend_plan);
    output.push('\n');

    output.push_str("## Machine Instructions\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.machine_instructions.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.machine_instructions.instructions.len()
    ));
    output.push_str(&format!(
        "encoded bytes: {}\n",
        backend_plan.encoded_machine.bytes.len()
    ));
    output.push_str(&format!(
        "bytes: {}\n",
        backend_plan.encoded_machine.byte_count
    ));
    for (_, function) in backend_plan.machine_instructions.functions.iter() {
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

fn write_assigned_value_homes(output: &mut String, backend_plan: &BackendReportInput<'_>) {
    if backend_plan.assigned_target_operations.runtime_value_homes.is_empty() {
        output.push_str("homes: none\n");
        return;
    }

    output.push_str("homes:\n");
    for (handle, home) in backend_plan.assigned_target_operations.runtime_value_homes.iter() {
        let operand_handle = omega_core::arena::Handle::from_arena_index(handle.arena_index());
        let operand = backend_plan
            .assigned_target_operations
            .runtime_value_operands
            .get(operand_handle);
        output.push_str(&format!(
            "  - #{} {} => {}\n",
            handle.arena_index(),
            runtime_value_operand_name(backend_plan, operand_handle),
            assigned_value_home_name(home.kind, operand)
        ));
    }
}

fn write_function_instruction_plan(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
    function: &TargetOperationFunction,
) {
    let source_name = backend_state_name(backend_plan, function.source_key);
    output.push_str(&format!(
        "- function {} from {}\n",
        function.symbol, source_name
    ));

    match backend_plan
        .target_operations
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
    instruction: &TargetOperation,
) {
    output.push_str(&format!(
        "    - statement {}: {}\n",
        instruction.source_statement,
        selected_instruction_name(backend_plan, instruction)
    ));
}

fn selected_instruction_name(
    backend_plan: &BackendReportInput<'_>,
    instruction: &TargetOperation,
) -> String {
    let kind = &instruction.kind;
    match kind {
        TargetOperationKind::EnterFunction => "enter function".to_owned(),
        TargetOperationKind::EnterDispatchLoop {
            entry_dispatch_index,
            terminal_dispatch_index,
        } => format!(
            "enter dispatch loop entry #{entry_dispatch_index} terminal #{terminal_dispatch_index}"
        ),
        TargetOperationKind::EnterDispatchCase { dispatch_index } => {
            let label = backend_plan
                .runtime_dispatch_loop
                .cases
                .iter()
                .find(|(_, dispatch_case)| dispatch_case.dispatch_index == *dispatch_index)
                .map(|(_, dispatch_case)| state_dispatch_label(dispatch_case.key))
                .unwrap_or_else(|| "unknown".to_owned());
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            format!("compare runtime text `{buffer_symbol}` with {literal:?}")
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_region,
            source_offset,
            operator,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
                runtime_value_operand_name(backend_plan, *left),
                runtime_value_operand_name(backend_plan, *right),
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { buffer, literal } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            format!("write runtime text `{buffer_symbol}` = {literal:?}")
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            buffer,
            byte_offset,
            literal,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
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
                runtime_value_operand_name(backend_plan, *left),
                runtime_value_operand_name(backend_plan, *right),
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
                runtime_value_operand_name(backend_plan, *left),
                runtime_value_operand_name(backend_plan, *right),
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
                runtime_value_operand_name(backend_plan, *left),
                runtime_value_operand_name(backend_plan, *right),
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
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
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
            format!(
                "write runtime pointee string runtime_frame@{pointer_byte_offset} +{field_byte_offset} data `{data_symbol}` len {byte_length}"
            )
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
            format!(
                "write runtime-frame indexed string descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} data `{data_symbol}` len {byte_length}"
            )
        }
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region,
            source_offset,
            target_offset,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            format!(
                "write runtime-frame pointer @{target_offset} = &{source_symbol}@{source_offset}"
            )
        }
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
        } => format!(
            "write runtime-frame pointer @{target_offset} = *(runtime_frame@{pointer_byte_offset}) +{field_byte_offset}"
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            target_offset,
            byte_capacity,
            source,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "read runtime text line {} -> `{buffer_symbol}` cap {byte_capacity}, descriptor {target_symbol}@{target_offset}",
                runtime_text_read_source_name(backend_plan, source)
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
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        } => {
            format!(
                "copy runtime-frame indexed descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} -> runtime_frame@{target_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        } => {
            format!(
                "copy runtime-frame fixed-indexed descriptor@{descriptor_offset} index {element_index} elem {element_byte_size} field +{field_byte_offset} -> runtime_frame@{target_offset} bytes {byte_count}"
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
                .map(|(_, call)| host_call_display_name(backend_plan, call))
                .unwrap_or_else(|| "unknown".to_owned());
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
        SelectedInstructionKind::WriteReturnRegisterInteger { byte_size, value } => {
            format!("write return register integer bytes {byte_size} value {value}")
        }
        SelectedInstructionKind::LeaveFunction => "leave function".to_owned(),
    }
}

fn runtime_text_read_source_name(
    backend_plan: &BackendReportInput<'_>,
    source: &omega_target_operations::RuntimeTextReadSource,
) -> String {
    match source {
        omega_target_operations::RuntimeTextReadSource::HostOperation { operation_key } => {
            match backend_plan
                .target_operations
                .host_binding(*operation_key)
                .map(|binding| &binding.mechanism)
            {
                Some(omega_calling_conventions::HostBindingMechanism::Import {
                    symbol,
                    ..
                }) => {
                    format!("import {symbol}")
                }
                Some(omega_calling_conventions::HostBindingMechanism::Syscall {
                    number,
                    number_register,
                    supervisor_call,
                    ..
                }) => {
                    format!("syscall {number} via x{number_register}/svc #{supervisor_call}")
                }
                None => {
                    format!(
                        "unresolved host operation {}.{}",
                        operation_key.capability_name(),
                        operation_key.operation_name()
                    )
                }
            }
        }
    }
}

fn runtime_value_operand_name(
    backend_plan: &BackendReportInput<'_>,
    operand: RuntimeValueOperandHandle,
) -> String {
    match backend_plan
        .target_operations
        .runtime_value_operands
        .get(operand)
    {
        omega_target_operations::RuntimeValueOperand::Immediate(value) => value.to_string(),
        omega_target_operations::RuntimeValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => {
            let symbol = storage_region_symbol_name(*region, backend_plan.entry_machine_name());
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
            runtime_value_operand_name(backend_plan, *left),
            runtime_value_operand_name(backend_plan, *right),
        ),
    }
}

fn assigned_value_home_name(
    home: omega_assigned_target_operations::AssignedValueHomeKind,
    operand: &omega_target_operations::RuntimeValueOperand,
) -> String {
    match home {
        omega_assigned_target_operations::AssignedValueHomeKind::Immediate => {
            "immediate".to_owned()
        }
        omega_assigned_target_operations::AssignedValueHomeKind::StackSlot {
            byte_offset,
            byte_size,
        } => format!("stack slot frame@{byte_offset}/{}", byte_size),
        omega_assigned_target_operations::AssignedValueHomeKind::RuntimeStorage {
            region,
            byte_offset,
            byte_size,
        } => format!("storage {region:?}@{byte_offset}/{}", byte_size),
        omega_assigned_target_operations::AssignedValueHomeKind::RuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => format!(
            "pointee frame@{pointer_byte_offset}+{field_byte_offset}/{}",
            byte_size
        ),
        omega_assigned_target_operations::AssignedValueHomeKind::RuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => format!(
            "frame-indexed desc@{descriptor_offset} idx@{index_offset} elem {element_byte_size} field +{field_byte_offset}/{}",
            byte_size
        ),
        omega_assigned_target_operations::AssignedValueHomeKind::ScratchRegister {
            bank,
            name,
        } => {
            let source = match operand {
                omega_target_operations::RuntimeValueOperand::Binary { .. } => "binary temp",
                _ => "temp",
            };
            format!("{bank:?} register {} ({source})", assigned_register_name(name))
        }
    }
}

fn assigned_register_name(name: omega_assigned_target_operations::AssignedRegisterName) -> String {
    match name {
        omega_assigned_target_operations::AssignedRegisterName::Aarch64X(register) => {
            format!("x{register}")
        }
        omega_assigned_target_operations::AssignedRegisterName::X86_64(register) => match register {
            omega_assigned_target_operations::X86_64AssignedRegister::R10 => "r10".to_owned(),
            omega_assigned_target_operations::X86_64AssignedRegister::R11 => "r11".to_owned(),
            omega_assigned_target_operations::X86_64AssignedRegister::R12 => "r12".to_owned(),
            omega_assigned_target_operations::X86_64AssignedRegister::R13 => "r13".to_owned(),
            omega_assigned_target_operations::X86_64AssignedRegister::R14 => "r14".to_owned(),
            omega_assigned_target_operations::X86_64AssignedRegister::R15 => "r15".to_owned(),
        },
    }
}

fn selected_instruction_operands_name(
    backend_plan: &BackendReportInput<'_>,
    operands: omega_core::arena::HandleSpan<InstructionOperand>,
) -> String {
    let Some(operands) = backend_plan.target_operations.operands.span(operands) else {
        return "invalid operands".to_owned();
    };

    operands
        .iter()
        .map(|operand| match &operand.kind {
            InstructionOperandKind::DataAddress { data } => {
                let symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
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
    function: &MachineInstructionFunction,
) {
    let function_symbol = machine_function_symbol(backend_plan, function);
    output.push_str(&format!("- function {}\n", function_symbol));

    match backend_plan
        .machine_instructions
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
        .functions
        .iter()
        .find(|(_, instruction_function)| instruction_function.source_key == function.source_key)
        .map(|(_, instruction_function)| instruction_function.symbol.to_string())
        .unwrap_or_else(|| format!("{:?}", function.source_key))
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
