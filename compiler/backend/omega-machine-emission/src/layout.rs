use crate::MachineEmissionContext;
use crate::host_bindings::host_binding_mechanism;
use omega_calling_conventions::HostBindingMechanism;
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection::{
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    dispatch_loop_enter_width, dispatch_state_write_width, host_call_sequence_width, return_width,
    runtime_frame_indexed_binary_write_width,
    runtime_frame_indexed_integer_write_width, runtime_machine_integer_write_width,
    runtime_machine_string_write_width,
    runtime_storage_binary_write_width,
    runtime_storage_compare_width, runtime_storage_copy_width, runtime_storage_value_compare_width,
    runtime_storage_copy_to_runtime_frame_indexed_width,
    runtime_text_buffer_materialize_width, runtime_text_line_read_width,
    runtime_text_literal_append_width, runtime_text_literal_compare_width,
    runtime_text_literal_segment_write_width, runtime_text_literal_write_width,
    runtime_text_storage_compare_width, runtime_text_stored_place_append_width,
    runtime_text_stored_suffix_append_width, syscall_sequence_width,
};
use omega_machine_program::{MachineInstruction, MachineInstructionKind};
use omega_target_operations::{SelectedInstructionKind, StateGuardLowering, StateGuardOperator};

#[derive(Debug, Clone)]
pub(crate) struct LaidOutMachineInstruction {
    pub selected_instruction_index: u32,
    pub offset: usize,
    pub byte_width: usize,
    pub kind: MachineInstructionKind,
}

pub(crate) fn layout_machine_instructions(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[MachineInstruction],
) -> Result<Vec<LaidOutMachineInstruction>, Diagnostic> {
    let mut laid_out = Vec::with_capacity(machine_instructions.len());
    let mut offset = 0usize;

    for machine_instruction in machine_instructions {
        let selected_handle =
            omega_core::arena::Handle::from_arena_index(machine_instruction.selected_instruction_index);
        let selected_instruction = input.instructions.instructions.get(selected_handle);
        let byte_width = machine_instruction_width(input, &selected_instruction.kind)?;

        laid_out.push(LaidOutMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            offset,
            byte_width,
            kind: machine_instruction.kind.clone(),
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
            let operands = input.instructions.operands.span(*operands).unwrap_or(&[]);
            match host_binding_mechanism(input, *operation_key) {
                Some(HostBindingMechanism::Syscall { number, .. }) => {
                    syscall_sequence_width(input.target.architecture, operands, *number)
                }
                _ => host_call_sequence_width(input.target.architecture, operands),
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
        SelectedInstructionKind::AppendRuntimeTextStoredPlace { .. } => {
            runtime_text_stored_place_append_width(input.target.architecture)
        }
        SelectedInstructionKind::AppendRuntimeTextLiteral { literal, .. } => {
            runtime_text_literal_append_width(input.target.architecture, literal)
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger { byte_size, .. } => {
            runtime_machine_integer_write_width(input.target.architecture, *byte_size)
        }
        SelectedInstructionKind::WriteRuntimeStorageInteger { byte_size, .. } => {
            runtime_machine_integer_write_width(input.target.architecture, *byte_size)
        }
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            byte_size,
            left,
            right,
            ..
        } => runtime_storage_binary_write_width(
            input.target.architecture,
            *byte_size,
            left,
            right,
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
        SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            right,
            ..
        } => runtime_frame_indexed_binary_write_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            left,
            right,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString { byte_length, .. } => {
            runtime_machine_string_write_width(input.target.architecture, *byte_length)
        }
        SelectedInstructionKind::ReadRuntimeTextLine {
            byte_capacity,
            source,
            ..
        } => runtime_text_line_read_width(input.target.architecture, *byte_capacity, source),
        SelectedInstructionKind::CopyRuntimeStorage { byte_count, .. } => {
            runtime_storage_copy_width(input.target.architecture, *byte_count)
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
            element_byte_size,
            field_byte_offset,
            byte_count,
            ..
        } => runtime_storage_copy_to_runtime_frame_indexed_width(
            input.target.architecture,
            *element_byte_size,
            *field_byte_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { .. }
        | SelectedInstructionKind::TerminateDispatch => {
            dispatch_state_write_width(input.target.architecture)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            dispatch_case_leave_width(input.target.architecture)
        }
        SelectedInstructionKind::LeaveFunction => return_width(input.target.architecture),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall => 0,
    })
}
