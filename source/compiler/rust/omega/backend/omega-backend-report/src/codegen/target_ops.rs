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
    let source_name = function
        .identity
        .source_key()
        .map(|source_key| backend_state_name(backend_plan, source_key))
        .unwrap_or_else(|| format!("{:?}", function.identity));
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
        TargetOperationKind::AtomicLoad {
            source_region,
            source_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
        } => format!(
            "atomic load {source_region:?}[{source_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicStore {
            target_region,
            target_offset,
            byte_size,
            ordering,
            ..
        } => format!(
            "atomic store {target_region:?}[{target_offset}] ({byte_size}B) ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicFetchAdd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic fetch_add {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicFetchSub {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic fetch_sub {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicFetchXor {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic fetch_xor {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicFetchOr {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic fetch_or {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicFetchAnd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic fetch_and {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicSwap {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic swap {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] ({})",
            ordering.success().name()
        ),
        TargetOperationKind::AtomicCompareExchange {
            target_region,
            target_offset,
            byte_size,
            result_region,
            result_offset,
            ordering,
            ..
        } => format!(
            "atomic compare_exchange {target_region:?}[{target_offset}] ({byte_size}B) -> \
             {result_region:?}[{result_offset}] (success {}, failure {})",
            ordering.success().name(),
            ordering
                .failure()
                .expect("compare_exchange operation carries a failure ordering")
                .name()
        ),
        TargetOperationKind::EnterFunction => "enter function".to_owned(),
        TargetOperationKind::WriteEntryArgumentRegister {
            register,
            byte_offset,
            byte_size,
        } => format!("entry prologue: {register:?} -> Frame[{byte_offset}] ({byte_size}B)"),
        TargetOperationKind::WriteEntryStackArgument {
            stack_byte_offset,
            byte_offset,
            byte_size,
        } => format!(
            "entry prologue: Stack[{stack_byte_offset}] -> Frame[{byte_offset}] ({byte_size}B)"
        ),
        TargetOperationKind::WriteEntryIndirectArgument {
            pointer,
            byte_offset,
            byte_size,
        } => format!("entry prologue: *{pointer:?} -> Frame[{byte_offset}] ({byte_size}B)"),
        TargetOperationKind::WriteEntryArgumentsSliceDescriptor {
            descriptor_offset,
            spill_offset,
            byte_length,
        } => format!(
            "entry prologue: args descriptor Frame[{descriptor_offset}] = {{ptr Frame[{spill_offset}], len {byte_length}}}"
        ),
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
        SelectedInstructionKind::ComparePlaces {
            left,
            right,
            byte_size,
            operator,
            ..
        } => {
            let left_symbol =
                storage_region_symbol_name(left.region, backend_plan.entry_machine_name());
            let right_symbol =
                storage_region_symbol_name(right.region, backend_plan.entry_machine_name());
            format!(
                "compare places {left_symbol}{:?} {operator:?} {right_symbol}{:?} bytes {byte_size}",
                left.steps(),
                right.steps()
            )
        }
        SelectedInstructionKind::ComparePlaceValue {
            place,
            byte_size,
            expected_value,
            operator,
        } => {
            let symbol =
                storage_region_symbol_name(place.region, backend_plan.entry_machine_name());
            format!(
                "compare place {symbol}{:?} {operator:?} {expected_value} bytes {byte_size}",
                place.steps()
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
        SelectedInstructionKind::MaterializeTextBufferToPlace { buffer, target } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            format!(
                "materialize text buffer `{buffer_symbol}` -> {target_symbol}{:?}",
                target.steps()
            )
        }
        SelectedInstructionKind::AppendTextStoredToPlace {
            buffer,
            source_region,
            source_offset,
            target,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            let source_symbol =
                storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            format!(
                "append text stored {source_symbol}@{source_offset} via `{buffer_symbol}` -> {target_symbol}{:?}",
                target.steps()
            )
        }
        SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target,
            literal,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            format!(
                "append text literal {literal:?} via `{buffer_symbol}` -> {target_symbol}{:?}",
                target.steps()
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
        SelectedInstructionKind::AppendWireScalarSlice {
            source_region,
            source_offset,
            element_byte_size,
            zigzag,
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
            let encoding = if *zigzag { "zigzag varints" } else { "varints" };
            format!(
                "wire append borrowed scalar slice ({encoding}, {element_byte_size}-byte elements, exact two-pass length) {source_symbol}@{source_offset} descriptor -> {out_symbol}@{out_offset} (cap {out_length}) + cursor {written_symbol}@{written_offset}"
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
            range,
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
            let range = range.map_or_else(String::new, |range| {
                format!(
                    ", establish {} [{}..={}]",
                    if range.signed { "signed" } else { "unsigned" },
                    range.minimum,
                    range.maximum
                )
            });
            format!(
                "wire read {encoding} {target_symbol}@{target_offset} ({byte_size} bytes) <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}{range}"
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
            predicate_mask,
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
                "wire read byte-slice view {target_symbol}@{target_offset} <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}, predicate mask {predicate_mask:#x}"
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
            range,
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
            let establishment = range.map_or_else(String::new, |range| {
                let algebra = if range.signed { "signed" } else { "unsigned" };
                format!(
                    ", establish {algebra} [{}..={}]",
                    range.minimum, range.maximum
                )
            });
            format!(
                "wire read repeated {encoding} {target_symbol}@{target_offset} ({byte_size} bytes){establishment} while cursor < end {end_symbol}@{end_offset}, count {count_symbol}@{count_offset} += 1 <- {buffer_symbol}@{buffer_offset} (len {buffer_length}) + cursor {read_symbol}@{read_offset}, ok {ok_symbol}@{ok_offset}"
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
        SelectedInstructionKind::WritePlaceConvert {
            target,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            ..
        } => format!(
            "write place convert {target:?} bytes {target_byte_size} float={target_is_float} <- {} bytes {source_byte_size} float={source_is_float}",
            runtime_value_operand_name(backend_plan, *source),
        ),
        SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source } => {
            format!("append place bounded buffer source target={target:?} source={source:?}")
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, literal } => {
            format!("append place bounded buffer literal target={target:?} {literal:?}")
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            buffer,
            target_region,
            target_offset,
            byte_capacity,
            source,
            target,
        } => {
            let buffer_symbol = backend_plan.data.objects.get(*buffer).symbol.as_ref();
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            match target {
                omega_target_operations::RuntimeTextReadTarget::StringDescriptor => format!(
                    "read runtime text line {} -> `{buffer_symbol}` cap {byte_capacity}, descriptor {target_symbol}@{target_offset}",
                    runtime_text_read_source_name(backend_plan, source)
                ),
                omega_target_operations::RuntimeTextReadTarget::BoundedByteBuffer => format!(
                    "read runtime text line {} -> carrier {target_symbol}@{target_offset} cap {byte_capacity}",
                    runtime_text_read_source_name(backend_plan, source)
                ),
                omega_target_operations::RuntimeTextReadTarget::FixedByteArray => format!(
                    "read runtime text line {} -> fixed byte array {target_symbol}@{target_offset} cap {byte_capacity}",
                    runtime_text_read_source_name(backend_plan, source)
                ),
            }
        }
        SelectedInstructionKind::ReadRuntimeByte {
            target_region,
            target_offset,
            payload_offset,
            source,
        } => {
            let target_symbol =
                storage_region_symbol_name(*target_region, backend_plan.entry_machine_name());
            format!(
                "read runtime byte {} -> ByteRead {target_symbol}@{target_offset} (payload +{payload_offset})",
                runtime_text_read_source_name(backend_plan, source)
            )
        }
        SelectedInstructionKind::WriteRuntimeByte {
            source_region,
            source_offset,
            literal,
            source_is_place,
            source,
        } => {
            if *source_is_place {
                let source_symbol =
                    storage_region_symbol_name(*source_region, backend_plan.entry_machine_name());
                format!(
                    "write runtime byte {} <- {source_symbol}@{source_offset}",
                    runtime_text_read_source_name(backend_plan, source)
                )
            } else {
                let literal_symbol = backend_plan.data.objects.get(*literal).symbol.as_ref();
                format!(
                    "write runtime byte {} <- literal `{literal_symbol}`",
                    runtime_text_read_source_name(backend_plan, source)
                )
            }
        }
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            role,
        } => {
            let source_symbol =
                storage_region_symbol_name(source.region, backend_plan.entry_machine_name());
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            let role = match role {
                omega_target_operations::CopyPlacesRole::Ordinary => "",
                omega_target_operations::CopyPlacesRole::ExitIndirectResult => {
                    " role exit_indirect_result"
                }
            };
            format!(
                "copy places {source_symbol}{:?} -> {target_symbol}{:?} bytes {byte_count}{role}",
                source.steps(),
                target.steps()
            )
        }
        SelectedInstructionKind::WritePlaceInteger {
            target,
            value,
            byte_size,
        } => {
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            format!(
                "write place integer {value} ({byte_size}b) -> {target_symbol}{:?}",
                target.steps()
            )
        }
        SelectedInstructionKind::WriteStorageBitField {
            region,
            base_byte_offset,
            fragments,
            value,
        } => {
            let target_symbol =
                storage_region_symbol_name(*region, backend_plan.entry_machine_name());
            format!(
                "write bit field {value} -> {target_symbol}@{base_byte_offset} ({} fragments)",
                fragments.len()
            )
        }
        SelectedInstructionKind::WritePlaceString {
            target,
            data,
            byte_length,
        } => {
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
            format!(
                "write place string data `{data_symbol}` len {byte_length} -> {target_symbol}{:?}",
                target.steps()
            )
        }
        SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } => {
            let source_symbol =
                storage_region_symbol_name(source.region, backend_plan.entry_machine_name());
            format!(
                "write place address &{source_symbol}{:?} -> frame[{target_offset}]",
                source.steps()
            )
        }
        SelectedInstructionKind::WriteDataAddressToRuntimeFrame {
            data,
            target_offset,
        } => {
            let data_symbol = backend_plan.data.objects.get(*data).symbol.as_ref();
            format!("write data address `{data_symbol}` -> frame[{target_offset}]")
        }
        SelectedInstructionKind::WritePlaceBoundedBuffer { target, literal } => {
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            format!(
                "write place bounded buffer {literal:?} -> {target_symbol}{:?}",
                target.steps()
            )
        }
        SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        } => {
            let target_symbol =
                storage_region_symbol_name(target.region, backend_plan.entry_machine_name());
            format!(
                "write place binary {left:?} {operator:?} {right:?} ({byte_size}b, float {is_float}, {domain:?}, signed {target_signed}) -> {target_symbol}{:?}",
                target.steps()
            )
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            format!("set dispatch state #{dispatch_index}")
        }
        SelectedInstructionKind::TerminateDispatch => "terminate dispatch".to_owned(),
        SelectedInstructionKind::LeaveDispatchCase => "leave dispatch case".to_owned(),
        SelectedInstructionKind::LeaveDispatchLoop => "leave dispatch loop".to_owned(),
        SelectedInstructionKind::CallInternalFunction { target } => {
            format!("call internal function {target:?}")
        }
        SelectedInstructionKind::LoadOutgoingStackAddress {
            register,
            stack_byte_offset,
        } => format!("load outgoing stack address rsp+{stack_byte_offset} -> {register:?}"),
        SelectedInstructionKind::ReserveOutgoingStackFrame { byte_count } => {
            format!("reserve outgoing stack frame ({byte_count} bytes)")
        }
        SelectedInstructionKind::WriteOutgoingStackU64 {
            stack_byte_offset,
            value,
        } => format!("write outgoing stack u64 {value:#x} at rsp+{stack_byte_offset}"),
        SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack {
            source_register,
            source_byte_offset,
            stack_byte_offset,
        } => format!(
            "copy entry indirect u64 {source_register:?}+{source_byte_offset} to rsp+{stack_byte_offset}"
        ),
        SelectedInstructionKind::ReleaseOutgoingStackFrame { byte_count } => {
            format!("release outgoing stack frame ({byte_count} bytes)")
        }
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
            ..
        } => {
            format!(
                "call host operation {}.{}({})",
                operation_key.capability_name(),
                operation_key.operation_name(),
                selected_instruction_operands_name(backend_plan, *operands)
            )
        }
        SelectedInstructionKind::DynamicTableCall {
            byte_offset,
            operands,
            ..
        } => format!(
            "call private dynamic table slot +{byte_offset}({})",
            selected_instruction_operands_name(backend_plan, *operands)
        ),
        SelectedInstructionKind::WriteReturnRegisterInteger {
            register,
            byte_size,
            value,
        } => {
            format!("write {register:?} integer bytes {byte_size} value {value}")
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            register,
            region,
            byte_offset,
            byte_size,
        } => {
            let region_symbol =
                storage_region_symbol_name(*region, backend_plan.entry_machine_name());
            format!(
                "copy runtime storage {region_symbol}@{byte_offset} bytes {byte_size} to {register:?}"
            )
        }
        SelectedInstructionKind::MachineHalt => "machine halt (hlt)".to_owned(),
        SelectedInstructionKind::MemoryFence(kind) => {
            format!("memory fence ({})", kind.mnemonic())
        }
        SelectedInstructionKind::InterruptControl(kind) => {
            format!("interrupt control ({})", kind.mnemonic())
        }
        SelectedInstructionKind::FlagsSnapshot {
            dest_byte_offset, ..
        } => format!("RFLAGS snapshot -> +{dest_byte_offset}"),
        SelectedInstructionKind::FlagsRestore { .. } => "RFLAGS restore".to_owned(),
        SelectedInstructionKind::MsrRead {
            dest_byte_offset, ..
        } => format!("model-specific register read (rdmsr) -> [{dest_byte_offset}]"),
        SelectedInstructionKind::MsrWrite { .. } => {
            "model-specific register write (wrmsr)".to_owned()
        }
        SelectedInstructionKind::ControlRegisterRead {
            register,
            dest_byte_offset,
            ..
        } => format!(
            "control-register read ({}) -> [{dest_byte_offset}]",
            register.read_mnemonic()
        ),
        SelectedInstructionKind::ControlRegisterWrite { register, .. } => format!(
            "control-register write ({})",
            register
                .write_mnemonic()
                .expect("writable control register")
        ),
        SelectedInstructionKind::PortWrite { .. } => "port write (out)".to_owned(),
        SelectedInstructionKind::PortRead {
            dest_byte_offset, ..
        } => {
            format!("port read (in) -> [{dest_byte_offset}]")
        }
        SelectedInstructionKind::LeaveFunction => "leave function".to_owned(),
    }
}
