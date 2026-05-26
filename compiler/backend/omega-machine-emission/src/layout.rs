use crate::MachineEmissionContext;
use crate::host_bindings::host_binding_mechanism;
use omega_assigned_target_operations::{
    RuntimeTextReadSource, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};
use omega_calling_conventions::HostBindingMechanism;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection::{
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    dispatch_loop_enter_width, dispatch_state_write_width, function_enter_width,
    host_call_sequence_width, return_register_integer_write_width, return_width,
    runtime_frame_base_indexed_binary_write_width, runtime_frame_base_indexed_integer_write_width,
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_frame_indexed_string_write_width, runtime_machine_integer_write_width,
    runtime_machine_string_write_width, runtime_pointee_binary_write_width,
    runtime_pointee_integer_write_width, runtime_pointee_string_write_width,
    runtime_storage_binary_write_width, runtime_storage_compare_width,
    runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width,
    runtime_storage_copy_from_runtime_frame_fixed_indexed_width,
    runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width,
    runtime_storage_copy_from_runtime_frame_indexed_width,
    runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width,
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
    Ok(match kind {
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } => {
            let operands = input
                .assigned_target_operations
                .instruction_operands(*operands)
                .unwrap_or(&[]);
            match host_binding_mechanism(input, *operation_key) {
                Some(HostBindingMechanism::Syscall { number, .. }) => {
                    syscall_sequence_width(input.target.architecture, operands, *number)
                }
                _ => host_call_sequence_width(input.target.architecture, *operation_key, operands),
            }
        }
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
                | StateGuardOperator::LessOrEqual,
            has_storage: true,
            ..
        } => dispatch_guard_compare_static_width(input.target.architecture),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_compare_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage { .. } => {
            runtime_text_storage_compare_width(input.target.architecture)
        }
        SelectedInstructionKind::CompareRuntimeStorage { .. } => {
            runtime_storage_compare_width(input.target.architecture)
        }
        SelectedInstructionKind::CompareRuntimeStorageValue { .. } => {
            runtime_storage_value_compare_width(input.target.architecture)
        }
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
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix { .. } => {
            runtime_text_stored_suffix_append_width(input.target.architecture)
        }
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { .. } => {
            runtime_text_buffer_materialize_width(input.target.architecture)
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_buffer_materialize_to_runtime_pointee_width(
            input.target.architecture,
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
        SelectedInstructionKind::AppendRuntimeTextStoredPlace { .. } => {
            runtime_text_stored_place_append_width(input.target.architecture)
        }
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_stored_place_append_to_runtime_pointee_width(
            input.target.architecture,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_text_stored_place_append_to_runtime_frame_indexed_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_append_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            field_byte_offset,
            literal,
            ..
        } => omega_instruction_selection::runtime_text_literal_append_to_runtime_pointee_width(
            input.target.architecture,
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
            field_byte_offset,
            byte_size,
            ..
        } => runtime_pointee_integer_write_width(
            input.target.architecture,
            *field_byte_offset,
            *byte_size,
        ),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_storage_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
            *byte_size,
            *left,
            *operator,
            *right,
        ),
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
            ..
        } => runtime_pointee_binary_write_width(
            input.target.architecture,
            input.assigned_target_operations,
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
            element_byte_size,
            field_byte_offset,
            byte_size,
            ..
        } => runtime_frame_base_indexed_integer_write_width(
            input.target.architecture,
            *base_byte_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
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
        SelectedInstructionKind::WriteRuntimePointeeString {
            field_byte_offset,
            byte_length,
            ..
        } => runtime_pointee_string_write_width(
            input.target.architecture,
            *field_byte_offset,
            *byte_length,
        ),
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
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame { .. } => {
            omega_instruction_selection::runtime_storage_address_to_runtime_frame_write_width(
                input.target.architecture,
            )
        }
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame { .. } => {
            omega_instruction_selection::runtime_pointee_address_to_runtime_frame_write_width(
                input.target.architecture,
            )
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_frame_indexed_address_to_runtime_frame_write_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => omega_instruction_selection::runtime_frame_base_indexed_address_to_runtime_frame_write_width(
            input.target.architecture,
            *base_byte_offset,
            *element_byte_size,
            *field_byte_offset,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            byte_capacity,
            source,
            ..
        } => {
            let RuntimeTextReadSource::HostOperation { operation_key } = source;
            let Some(binding) = input.assigned_target_operations.host_binding(*operation_key) else {
                return Err(Diagnostic::error(format!(
                    "missing host binding for runtime text read operation {}.{}",
                    operation_key.capability_name(),
                    operation_key.operation_name()
                )));
            };
            runtime_text_line_read_width(
                input.target.architecture,
                *byte_capacity,
                &binding.mechanism,
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
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
            source_offset,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage_copy_to_runtime_pointee_width(
            input.target.architecture,
            *source_offset,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::TerminateDispatch => {
            dispatch_state_write_width(input.target.architecture)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger { .. } => {
            return_register_integer_write_width(input.target.architecture)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            dispatch_case_leave_width(input.target.architecture)
        }
        SelectedInstructionKind::EnterFunction => function_enter_width(input.target.architecture),
        SelectedInstructionKind::LeaveFunction => return_width(input.target.architecture),
        SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => 0,
    })
}
