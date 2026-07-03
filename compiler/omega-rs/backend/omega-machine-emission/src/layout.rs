use crate::MachineEmissionContext;
use crate::host_bindings::host_binding_mechanism;
use crate::selected_instruction_queries::{selected_host_operation, selected_host_text_read};
use omega_assigned_target_operations::{
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};
use omega_calling_conventions::HostBindingMechanism;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection::{
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    runtime_atomic_compare_exchange_width, runtime_atomic_fetch_add_width,
    dispatch_loop_enter_width, dispatch_state_write_width, function_enter_width,
    host_call_sequence_width, return_register_integer_write_width, return_width,
    runtime_frame_base_indexed_binary_write_width, runtime_frame_base_indexed_integer_write_width,
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_frame_indexed_string_write_width, runtime_frame_string_write_width,
    runtime_machine_bounded_buffer_literal_append_width,
    runtime_machine_bounded_buffer_source_append_width, runtime_machine_bounded_buffer_write_width,
    runtime_machine_indexed_binary_write_width, runtime_machine_integer_write_width,
    runtime_machine_string_write_width,
    runtime_pointee_binary_write_width, runtime_pointee_integer_write_width,
    runtime_pointee_bounded_buffer_write_width, runtime_pointee_string_write_width,
    runtime_storage_binary_write_width,
    runtime_storage_compare_width, runtime_storage_convert_width,
    runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width,
    runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width,
    runtime_storage_copy_from_runtime_frame_fixed_indexed_width,
    runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width,
    runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width,
    runtime_storage_copy_from_runtime_frame_indexed_width,
    runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width,
    runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width,
    runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width,
    entry_argument_register_write_width,
    runtime_storage_copy_to_return_register_width,
    runtime_storage_copy_to_runtime_frame_indexed_width,
    runtime_storage_copy_to_runtime_pointee_width, runtime_storage_copy_width,
    runtime_storage_value_compare_width, runtime_text_buffer_materialize_width,
    runtime_text_line_read_width, runtime_text_literal_append_width,
    runtime_text_literal_compare_width, runtime_text_literal_segment_write_width,
    runtime_text_literal_write_width, runtime_text_storage_compare_width,
    runtime_text_stored_place_append_width, runtime_text_stored_suffix_append_width,
    runtime_value_compare_width, syscall_sequence_width,
};
use omega_machine_instructions::{MachineInstruction, MachineInstructionKind};

#[derive(Debug, Clone)]
pub(crate) struct LaidOutMachineInstruction {
    pub selected_instruction_index: u32,
    pub offset: usize,
    pub byte_width: usize,
    pub kind: MachineInstructionKind,
    pub source_kind: SelectedInstructionKind,
}

pub(crate) fn layout_machine_instructions(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[MachineInstruction],
) -> Result<Vec<LaidOutMachineInstruction>, Diagnostic> {
    let mut laid_out = Vec::with_capacity(machine_instructions.len());
    let mut offset = 0usize;

    for machine_instruction in machine_instructions {
        let byte_width = machine_instruction_width(input, &machine_instruction.source_kind)?;

        laid_out.push(LaidOutMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            offset,
            byte_width,
            kind: machine_instruction.kind,
            source_kind: machine_instruction.source_kind.clone(),
        });
        offset += byte_width;
    }

    Ok(laid_out)
}

fn machine_instruction_width(
    input: MachineEmissionContext<'_>,
    kind: &SelectedInstructionKind,
) -> Result<usize, Diagnostic> {
    if let Some(host_operation) = selected_host_operation(kind) {
        let operands = input
            .assigned_target_operations
            .instruction_operands(host_operation.operands)
            .unwrap_or(&[]);
        let width = match host_binding_mechanism(input, host_operation.operation_key) {
            Some(HostBindingMechanism::Syscall { number, .. }) => {
                syscall_sequence_width(input.target.architecture, operands, *number)
            }
            _ => host_call_sequence_width(
                input.target.architecture,
                host_operation.operation_key,
                operands,
            ),
        };
        // A host call is never legitimately empty: a zero width means the
        // encoder rejected the operands (e.g. an argument that failed to
        // marshal). Refuse loudly -- a zero-byte call would still have its
        // import relocation applied, which corrupts whatever instruction
        // lands at that offset and crashes the binary at runtime.
        if width == 0 {
            return Err(Diagnostic::error(format!(
                "host operation {}.{} has no encodable call sequence: a host-call argument \
                 must be a simple value (a local, field, parameter, or literal). Bind a computed \
                 argument -- arithmetic, a cast, or a value-call result -- to a local or field \
                 first, then pass that. (refusing to emit a zero-byte host call)",
                host_operation.operation_key.capability_name(),
                host_operation.operation_key.operation_name(),
            )));
        }
        return Ok(width);
    }

    Ok(match kind {
        SelectedInstructionKind::EnterDispatchLoop { .. } => {
            dispatch_loop_enter_width(input.target.architecture)
        }
        SelectedInstructionKind::EnterDispatchCase { .. } => {
            dispatch_case_enter_width(input.target.architecture)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual
                | StateGuardOperator::GreaterUnsigned
                | StateGuardOperator::GreaterOrEqualUnsigned
                | StateGuardOperator::LessUnsigned
                | StateGuardOperator::LessOrEqualUnsigned,
            has_storage: true,
            byte_offset,
            byte_size,
            is_float,
            ..
        } => dispatch_guard_compare_static_width(
            input.target.architecture,
            *byte_offset,
            *byte_size,
            *is_float,
        ),
        // A forward skip-jump is a plain unconditional `jmp rel32` -- same shape as a
        // dispatch-case leave, so it reuses that width/encoder.
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::ForwardBranchSkip,
            ..
        } => dispatch_case_leave_width(input.target.architecture),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_compare_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer,
            source_offset,
            ..
        } => {
            let literal_len = input.data.objects.get(*buffer).bytes.len();
            runtime_text_storage_compare_width(input.target.architecture, *source_offset, literal_len)
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            is_float,
            ..
        } => runtime_storage_compare_width(
            input.target.architecture,
            *left_offset,
            *right_offset,
            *byte_size,
            *is_float,
        ),
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            ..
        } => runtime_storage_value_compare_width(
            input.target.architecture,
            *byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::CompareRuntimeValues {
            left,
            right,
            byte_size,
            ..
        } => runtime_value_compare_width(
            input.target.architecture,
            input.assigned_target_operations,
            *byte_size,
            *left,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_write_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { literal, .. } => {
            runtime_text_literal_segment_write_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => runtime_text_stored_suffix_append_width(
            input.target.architecture,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            runtime_text_buffer_materialize_width(input.target.architecture, *target_offset)
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_buffer_materialize_to_runtime_pointee_width(
            input.target.architecture,
            *pointer_byte_offset,
            *field_byte_offset,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => {
            runtime_text_stored_place_append_width(input.target.architecture, *source_offset, *target_offset)
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_stored_place_append_to_runtime_pointee_width(
            input.target.architecture,
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            source_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_stored_place_append_to_runtime_frame_indexed_width(
            input.target.architecture,
            *source_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => {
            runtime_text_literal_append_width(input.target.architecture, *target_offset, literal)
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            literal,
            ..
        } => omega_instruction_selection::runtime_text_literal_append_to_runtime_pointee_width(
            input.target.architecture,
            *pointer_byte_offset,
            *field_byte_offset,
            literal,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            element_byte_size,
            field_byte_offset,
            literal,
            ..
        } => omega_instruction_selection::runtime_text_literal_append_to_runtime_frame_indexed_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            literal,
        ),
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            ..
        } => runtime_machine_integer_write_width(
            input.target.architecture,
            *byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteRuntimeStorageInteger {
            byte_offset,
            byte_size,
            ..
        } => {
            runtime_machine_integer_write_width(input.target.architecture, *byte_offset, *byte_size)
        }
        SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            ..
        } => runtime_pointee_integer_write_width(
            input.target.architecture,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
            ..
        } => runtime_storage_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *left,
            *operator,
            *right,
            *is_float,
            *domain,
            *target_signed,
        ),
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            ..
        } => runtime_storage_convert_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *source,
            *source_byte_size,
            *target_byte_size,
            *source_is_float,
            *target_is_float,
            *source_signed,
        ),
        SelectedInstructionKind::AtomicFetchAdd {
            target_offset,
            byte_size,
            delta,
            ..
        } => runtime_atomic_fetch_add_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *delta,
        ),
        SelectedInstructionKind::AtomicCompareExchange {
            target_offset,
            byte_size,
            expected,
            new_value,
            ..
        } => runtime_atomic_compare_exchange_width(
            input.target.architecture,
            input.assigned_target_operations,
            *target_offset,
            *byte_size,
            *expected,
            *new_value,
        ),
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_pointee_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
            element_byte_size,
            field_byte_offset,
            byte_size,
            ..
        } => runtime_frame_indexed_integer_write_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            ..
        } => runtime_frame_base_indexed_integer_write_width(
            input.target.architecture,
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::AppendWireLiteralByte {
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_literal_byte_width(
            input.target.architecture,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireScalarVarint {
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_scalar_varint_width(
            input.target.architecture,
            *source_offset,
            *byte_size,
            *zigzag,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::AppendWireTextBytes {
            source_offset,
            out_offset,
            out_length,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_text_bytes_width(
            input.target.architecture,
            *source_offset,
            *out_offset,
            *out_length,
            *written_offset,
        ),
        SelectedInstructionKind::ReadWireExpectedByte {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            ..
        } => omega_instruction_selection::read_wire_expected_byte_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
        ),
        SelectedInstructionKind::ReadWireScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag,
            ..
        } => omega_instruction_selection::read_wire_scalar_varint_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_offset,
            *byte_size,
            *zigzag,
        ),
        SelectedInstructionKind::ReadWireByteSlice {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            ..
        } => omega_instruction_selection::read_wire_byte_slice_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *target_offset,
        ),
        SelectedInstructionKind::ReadWireNestedOpen {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            ..
        } => omega_instruction_selection::read_wire_nested_open_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::ReadWireNestedClose {
            buffer_offset,
            read_offset,
            ok_offset,
            end_offset,
            ..
        } => omega_instruction_selection::read_wire_nested_close_width(
            input.target.architecture,
            *buffer_offset,
            *read_offset,
            *ok_offset,
            *end_offset,
        ),
        SelectedInstructionKind::AppendWireRepeatedScalarVarint {
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset,
            ..
        } => omega_instruction_selection::append_wire_repeated_scalar_varint_width(
            input.target.architecture,
            *source_offset,
            *byte_size,
            *zigzag,
            *index,
            *count_offset,
            *out_offset,
            *written_offset,
        ),
        SelectedInstructionKind::ReadWireRepeatedScalarVarint {
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_offset,
            target_offset,
            byte_size,
            zigzag,
            ..
        } => omega_instruction_selection::read_wire_repeated_scalar_varint_width(
            input.target.architecture,
            *buffer_offset,
            *buffer_length,
            *read_offset,
            *ok_offset,
            *end_offset,
            *count_offset,
            *target_offset,
            *byte_size,
            *zigzag,
        ),
        SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset,
            index_region,
            element_byte_size,
            field_byte_offset,
            byte_size,
            ..
        } => omega_instruction_selection::runtime_machine_indexed_integer_write_width(
            input.target.architecture,
            *base_byte_offset,
            *index_region,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_frame_indexed_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_frame_base_indexed_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeMachineIndexedBinary {
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        } => runtime_machine_indexed_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
            *base_byte_offset,
            *index_region,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString { byte_length, .. } => {
            runtime_machine_string_write_width(input.target.architecture, *byte_length)
        }
        SelectedInstructionKind::WriteRuntimeMachineBoundedBuffer { literal, .. } => {
            runtime_machine_bounded_buffer_write_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferSource {
            source_in_frame,
            ..
        } => runtime_machine_bounded_buffer_source_append_width(
            input.target.architecture,
            *source_in_frame,
        ),
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferLiteral { literal, .. } => {
            runtime_machine_bounded_buffer_literal_append_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::WriteRuntimeFrameString { byte_length, .. } => {
            runtime_frame_string_write_width(input.target.architecture, *byte_length)
        }
        SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_pointee_string_write_width(
            input.target.architecture,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimePointeeBoundedBuffer { literal, .. } => {
            runtime_pointee_bounded_buffer_write_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => runtime_frame_indexed_string_write_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimeMachineIndexedString {
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => omega_instruction_selection::runtime_machine_indexed_string_write_width(
            input.target.architecture,
            *base_byte_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_offset,
            target_offset,
            ..
        } => {
            omega_instruction_selection::runtime_storage_address_to_runtime_frame_write_width(
                input.target.architecture,
                *source_offset,
                *target_offset,
            )
        }
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            ..
        } => {
            omega_instruction_selection::runtime_pointee_address_to_runtime_frame_write_width(
                input.target.architecture,
                *pointer_byte_offset,
                *field_byte_offset,
                *target_offset,
            )
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            element_byte_size,
            field_byte_offset,
            target_offset,
            ..
        } => omega_instruction_selection::runtime_frame_indexed_address_to_runtime_frame_write_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            ..
        } => omega_instruction_selection::runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
            input.target.architecture,
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            ..
        } => omega_instruction_selection::runtime_frame_base_indexed_address_to_runtime_frame_write_width(
            input.target.architecture,
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine { .. } => {
            let Some(read) = selected_host_text_read(kind) else {
                return Err(Diagnostic::error(
                    "cannot lay out runtime text read: missing host operation source",
                ));
            };
            let Some(binding) = input
                .assigned_target_operations
                .host_binding(read.operation_key)
            else {
                return Err(Diagnostic::error(format!(
                    "missing host binding for runtime text read operation {}.{}",
                    read.operation_key.capability_name(),
                    read.operation_key.operation_name()
                )));
            };
            runtime_text_line_read_width(
                input.target.architecture,
                read.byte_capacity,
                &binding.mechanism,
                read.is_bounded_buffer,
            )
        }
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage_copy_width(
            input.target.architecture,
            *source_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
            source_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage_copy_to_runtime_frame_indexed_width(
            input.target.architecture,
            *source_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_frame_indexed_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage {
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
            input.target.architecture,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
            input.target.architecture,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimePointee {
            element_index,
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
            input.target.architecture,
            *element_index,
            *element_byte_size,
            *source_field_byte_offset,
            *target_field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimePointee {
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
            input.target.architecture,
            *element_byte_size,
            *source_field_byte_offset,
            *target_field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
            input.target.architecture,
            *base_byte_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeMachineIndexed { .. } => {
            runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(
                input.target.architecture,
            )
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage_copy_to_runtime_pointee_width(
            input.target.architecture,
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            byte_count,
        } => runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
            input.target.architecture,
            *pointer_byte_offset,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::TerminateDispatch => {
            dispatch_state_write_width(input.target.architecture)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger { .. } => {
            return_register_integer_write_width(input.target.architecture)
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
            byte_offset,
            byte_size,
            ..
        } => runtime_storage_copy_to_return_register_width(
            input.target.architecture,
            *byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteEntryArgumentRegister { .. } => {
            entry_argument_register_write_width(input.target.architecture)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            dispatch_case_leave_width(input.target.architecture)
        }
        SelectedInstructionKind::EnterFunction => function_enter_width(input.target.architecture),
        SelectedInstructionKind::LeaveFunction => return_width(input.target.architecture),
        SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => 0,
        SelectedInstructionKind::HostOperation { .. } => {
            return Err(Diagnostic::error(
                "internal error: host operation was not handled by machine layout host query",
            ));
        }
    })
}
