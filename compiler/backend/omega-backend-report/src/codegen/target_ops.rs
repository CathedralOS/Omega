use super::super::backend_state_name;
use super::super::host::host_call_display_name;
use super::operands::selected_instruction_operands_name;
use super::runtime_values::{runtime_text_read_source_name, runtime_value_operand_name};

use crate::BackendReportInput;
use omega_object_file::storage_region_symbol_name;
use omega_state_dispatch::state_dispatch_label;
use omega_target_operations::{
    SelectedInstructionKind, TargetOperation, TargetOperationFunction, TargetOperationKind,
};

pub(super) fn write_target_operations_section(
    output: &mut String,
    backend_plan: &BackendReportInput<'_>,
) {
    output.push_str("## Target Operations\n");
    output.push_str(&format!(
        "functions: {}\n",
        backend_plan.target_operations.code.functions.len()
    ));
    output.push_str(&format!(
        "instructions: {}\n",
        backend_plan.target_operations.code.instructions.len()
    ));
    output.push_str(&format!(
        "operands: {}\n",
        backend_plan.target_operations.code.operands.len()
    ));
    for (_, function) in backend_plan.target_operations.code.functions.iter() {
        write_function_instruction_plan(output, backend_plan, function);
    }
    output.push('\n');
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
        .code
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
        TargetOperationKind::AtomicFetchAdd {
            target_region,
            target_offset,
            byte_size,
            ..
        } => format!("atomic fetch_add {target_region:?}[{target_offset}] ({byte_size}B)"),
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
            ..
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
            ..
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
        SelectedInstructionKind::AppendWireLiteralByte {
            out_region,
            out_offset,
            written_region,
            written_offset,
            value,
        } => {
            let out_symbol =
                storage_region_symbol_name(*out_region, backend_plan.entry_machine_name());
            let written_symbol =
                storage_region_symbol_name(*written_region, backend_plan.entry_machine_name());
            format!(
                "wire append literal byte {value:#04x} -> {out_symbol}@{out_offset} + cursor {written_symbol}@{written_offset}"
            )
        }
        SelectedInstructionKind::AppendWireScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            out_region,
            out_offset,
            written_region,
            written_offset,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let out_symbol =
                storage_region_symbol_name(*out_region, backend_plan.entry_machine_name());
            let written_symbol =
                storage_region_symbol_name(*written_region, backend_plan.entry_machine_name());
            let encoding = if *zigzag { "zigzag varint" } else { "varint" };
            format!(
                "wire append {encoding} {source_symbol}@{source_offset} ({byte_size} bytes) -> {out_symbol}@{out_offset} + cursor {written_symbol}@{written_offset}"
            )
        }
        SelectedInstructionKind::AppendWireTextBytes {
            source_region,
            source_offset,
            out_region,
            out_offset,
            out_length,
            written_region,
            written_offset,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let out_symbol =
                storage_region_symbol_name(*out_region, backend_plan.entry_machine_name());
            let written_symbol =
                storage_region_symbol_name(*written_region, backend_plan.entry_machine_name());
            format!(
                "wire append text bytes (len varint + raw bytes) {source_symbol}@{source_offset} descriptor -> {out_symbol}@{out_offset} (cap {out_length}) + cursor {written_symbol}@{written_offset}"
            )
        }
        SelectedInstructionKind::ReadWireExpectedByte {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            expected,
        } => {
            let buffer_symbol =
                storage_region_symbol_name(*buffer_region, backend_plan.entry_machine_name());
            let read_symbol =
                storage_region_symbol_name(*read_region, backend_plan.entry_machine_name());
            let ok_symbol =
                storage_region_symbol_name(*ok_region, backend_plan.entry_machine_name());
            format!(
                "wire read expect byte {expected:#04x} <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}"
            )
        }
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
        } => {
            let buffer_symbol =
                storage_region_symbol_name(*buffer_region, backend_plan.entry_machine_name());
            let read_symbol =
                storage_region_symbol_name(*read_region, backend_plan.entry_machine_name());
            let ok_symbol =
                storage_region_symbol_name(*ok_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            let encoding = if *zigzag { "zigzag varint" } else { "varint" };
            format!(
                "wire read {encoding} {target_symbol}@{target_offset} ({byte_size} bytes) <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}"
            )
        }
        SelectedInstructionKind::ReadWireByteSlice {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            target_region,
            target_offset,
        } => {
            let buffer_symbol =
                storage_region_symbol_name(*buffer_region, backend_plan.entry_machine_name());
            let read_symbol =
                storage_region_symbol_name(*read_region, backend_plan.entry_machine_name());
            let ok_symbol =
                storage_region_symbol_name(*ok_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "wire read byte-slice view {target_symbol}@{target_offset} <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}"
            )
        }
        SelectedInstructionKind::ReadWireNestedOpen {
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            end_region,
            end_offset,
            ..
        } => {
            let read_symbol =
                storage_region_symbol_name(*read_region, backend_plan.entry_machine_name());
            let ok_symbol =
                storage_region_symbol_name(*ok_region, backend_plan.entry_machine_name());
            let end_symbol =
                storage_region_symbol_name(*end_region, backend_plan.entry_machine_name());
            format!(
                "wire nested open: end bound {end_symbol}@{end_offset} += cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset} &= end <= {buffer_length}"
            )
        }
        SelectedInstructionKind::ReadWireNestedClose {
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            end_region,
            end_offset,
            ..
        } => {
            let read_symbol =
                storage_region_symbol_name(*read_region, backend_plan.entry_machine_name());
            let ok_symbol =
                storage_region_symbol_name(*ok_region, backend_plan.entry_machine_name());
            let end_symbol =
                storage_region_symbol_name(*end_region, backend_plan.entry_machine_name());
            format!(
                "wire nested close: ok {ok_symbol}@{ok_offset} &= cursor {read_symbol}@{read_offset} == end bound {end_symbol}@{end_offset}"
            )
        }
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_region,
            source_offset,
            byte_size,
            zigzag,
            index,
            count_region,
            count_offset,
            out_region,
            out_offset,
            written_region,
            written_offset,
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let count_symbol =
                storage_region_symbol_name(*count_region, backend_plan.entry_machine_name());
            let out_symbol =
                storage_region_symbol_name(*out_region, backend_plan.entry_machine_name());
            let written_symbol =
                storage_region_symbol_name(*written_region, backend_plan.entry_machine_name());
            let encoding = if *zigzag { "zigzag varint" } else { "varint" };
            format!(
                "wire append repeated {encoding} [{index}] {source_symbol}@{source_offset} ({byte_size} bytes) if {index} < count {count_symbol}@{count_offset} -> {out_symbol}@{out_offset} + cursor {written_symbol}@{written_offset}"
            )
        }
        SelectedInstructionKind::ReadWireRepeatedScalarVarint {
            buffer_region,
            buffer_offset,
            buffer_length,
            read_region,
            read_offset,
            ok_region,
            ok_offset,
            end_region,
            end_offset,
            count_region,
            count_offset,
            target_region,
            target_offset,
            byte_size,
            zigzag,
        } => {
            let buffer_symbol =
                storage_region_symbol_name(*buffer_region, backend_plan.entry_machine_name());
            let read_symbol =
                storage_region_symbol_name(*read_region, backend_plan.entry_machine_name());
            let ok_symbol =
                storage_region_symbol_name(*ok_region, backend_plan.entry_machine_name());
            let end_symbol =
                storage_region_symbol_name(*end_region, backend_plan.entry_machine_name());
            let count_symbol =
                storage_region_symbol_name(*count_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            let encoding = if *zigzag { "zigzag varint" } else { "varint" };
            format!(
                "wire read repeated {encoding} {target_symbol}@{target_offset} ({byte_size} bytes) while cursor < end {end_symbol}@{end_offset}, count {count_symbol}@{count_offset} += 1 <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}"
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
            ..
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "write runtime storage binary {target_symbol}@{target_offset} bytes {byte_size} {} {operator:?} {}",
                runtime_value_operand_name(backend_plan, *left),
                runtime_value_operand_name(backend_plan, *right),
            )
        }
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_region,
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            ..
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "write runtime storage convert {target_symbol}@{target_offset} bytes {target_byte_size} float={target_is_float} <- {} bytes {source_byte_size} float={source_is_float}",
                runtime_value_operand_name(backend_plan, *source),
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
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => {
            format!(
                "write runtime-frame base indexed integer base@{base_byte_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} bytes {byte_size} value {value}"
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => {
            format!(
                "write runtime-machine indexed integer machine@{base_byte_offset} index({index_region:?})@{index_offset} elem {element_byte_size} field +{field_byte_offset} bytes {byte_size} value {value}"
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
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => {
            format!(
                "write runtime-frame base indexed binary base@{base_byte_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} bytes {byte_size} {} {operator:?} {}",
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
        SelectedInstructionKind::WriteRuntimeFrameString {
            byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
            format!(
                "write runtime-frame string offset {byte_offset} data `{data_symbol}` len {byte_length}"
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
        SelectedInstructionKind::WriteRuntimeMachineIndexedString {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            data,
            byte_length,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
            format!(
                "write runtime-machine indexed string machine@{base_byte_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} data `{data_symbol}` len {byte_length}"
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
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => format!(
            "write runtime-frame pointer @{target_offset} = &(runtime_frame@{descriptor_offset}[runtime_frame@{index_offset} * {element_byte_size}]) +{field_byte_offset}"
        ),
        SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => format!(
            "write runtime-frame pointer @{target_offset} = &(runtime_frame@{descriptor_offset}[{element_index} * {element_byte_size}]) +{field_byte_offset}"
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => format!(
            "write runtime-frame pointer @{target_offset} = &(runtime_frame@{base_byte_offset}[runtime_frame@{index_offset} * {element_byte_size}]) +{field_byte_offset}"
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
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_region,
            target_offset,
            byte_count,
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "copy runtime-frame indexed descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} -> {target_symbol}@{target_offset} bytes {byte_count}"
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
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_region,
            target_offset,
            byte_count,
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "copy runtime-frame fixed-indexed descriptor@{descriptor_offset} index {element_index} elem {element_byte_size} field +{field_byte_offset} -> {target_symbol}@{target_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimePointee {
            descriptor_offset,
            element_index,
            element_byte_size,
            source_field_byte_offset,
            pointer_byte_offset,
            target_field_byte_offset,
            byte_count,
        } => {
            format!(
                "copy runtime-frame fixed-indexed descriptor@{descriptor_offset} index {element_index} elem {element_byte_size} field +{source_field_byte_offset} -> pointee pointer@{pointer_byte_offset} field +{target_field_byte_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimePointee {
            descriptor_offset,
            index_offset,
            element_byte_size,
            source_field_byte_offset,
            pointer_byte_offset,
            target_field_byte_offset,
            byte_count,
        } => {
            format!(
                "copy runtime-frame indexed descriptor@{descriptor_offset} index@{index_offset} elem {element_byte_size} field +{source_field_byte_offset} -> pointee pointer@{pointer_byte_offset} field +{target_field_byte_offset} bytes {byte_count}"
            )
        }
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_region,
            target_offset,
            byte_count,
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "copy runtime-machine indexed base@{base_byte_offset} index@{index_offset} elem {element_byte_size} field +{field_byte_offset} -> {target_symbol}@{target_offset} bytes {byte_count}"
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
        SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            byte_count,
        } => {
            format!(
                "copy runtime pointee runtime_frame@{pointer_byte_offset} +{field_byte_offset} -> runtime_frame@{target_offset} bytes {byte_count}"
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
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            region,
            byte_offset,
            byte_size,
        } => {
            let region_symbol =
                storage_region_symbol_name(*region, backend_plan.entry_machine_name());
            format!(
                "copy runtime storage {region_symbol}@{byte_offset} bytes {byte_size} to return register"
            )
        }
        SelectedInstructionKind::LeaveFunction => "leave function".to_owned(),
    }
}
