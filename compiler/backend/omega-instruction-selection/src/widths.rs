use crate::aarch64_call_operand;
use omega_calling_conventions::HostBindingMechanism;
use omega_calling_conventions::HostOperationKey;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::{
    InstructionOperandLike, RuntimeValueOperandHandle, RuntimeValueOperandSource,
    StateGuardOperator,
};

pub fn host_call_sequence_width<T: InstructionOperandLike>(
    architecture: Architecture,
    operation_key: HostOperationKey,
    operands: &[T],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::host_call_sequence_width_from_operands(
            operands.iter().map(aarch64_call_operand),
        ),
        Architecture::X86_64 => x86_64::host_call_sequence_width(operation_key, operands),
    }
}

pub fn syscall_sequence_width<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
    syscall_number: u32,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::syscall_sequence_width_from_operands(
            operands.iter().map(aarch64_call_operand),
            syscall_number,
        ),
        Architecture::X86_64 => operands.len() * 8 + 7,
    }
}

pub fn function_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::function_enter_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn return_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::return_width(),
        Architecture::X86_64 => x86_64::return_width(),
    }
}

pub fn return_register_integer_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::return_register_integer_write_width(),
        Architecture::X86_64 => x86_64::return_register_integer_write_width(),
    }
}

pub fn dispatch_loop_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_loop_enter_width(),
        Architecture::X86_64 => x86_64::dispatch_loop_enter_width(),
    }
}

pub fn dispatch_case_enter_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_case_enter_width(),
        Architecture::X86_64 => x86_64::dispatch_case_enter_width(),
    }
}

pub fn dispatch_state_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_state_write_width(),
        Architecture::X86_64 => x86_64::dispatch_state_write_width(),
    }
}

pub fn dispatch_case_leave_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_case_leave_width(),
        Architecture::X86_64 => x86_64::dispatch_case_leave_width(),
    }
}

pub fn dispatch_guard_compare_static_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::dispatch_guard_compare_static_width(),
        Architecture::X86_64 => x86_64::dispatch_guard_compare_static_width(),
    }
}

pub fn runtime_text_literal_compare_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_compare_width(literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_compare_width(literal),
    }
}

pub fn runtime_text_storage_compare_width(architecture: Architecture, literal_len: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            let _ = literal_len;
            aarch64::runtime_text_storage_compare_width()
        }
        Architecture::X86_64 => x86_64::runtime_text_storage_compare_width_x86(literal_len),
    }
}

/// Byte offset within a `CompareRuntimeTextStorage` of the failure branch the
/// emitter must target with the compare-failure distance.
pub fn runtime_text_storage_compare_failure_branch_offset(
    architecture: Architecture,
    literal_len: usize,
) -> usize {
    match architecture {
        // AArch64's per-byte mismatch conditional branch ends 40 bytes in.
        Architecture::Aarch64 => {
            let _ = literal_len;
            40
        }
        Architecture::X86_64 => {
            x86_64::runtime_text_storage_compare_failure_branch_offset(literal_len)
        }
    }
}

/// Byte offset of the delimiter-failure branch (aarch64 uses `width - 4`; on
/// x86_64 both failure paths funnel through the same trampoline jmp).
pub fn runtime_text_storage_compare_delimiter_branch_offset(
    architecture: Architecture,
    literal_len: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_storage_compare_width().saturating_sub(4)
        }
        Architecture::X86_64 => {
            x86_64::runtime_text_storage_compare_failure_branch_offset(literal_len)
        }
    }
}

pub fn runtime_storage_compare_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_compare_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_value_compare_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_value_compare_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_write_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_write_width(literal),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_segment_write_width(
    architecture: Architecture,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_segment_write_width(literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_segment_write_width(literal),
    }
}

pub fn runtime_text_stored_suffix_append_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_stored_suffix_append_width(),
        Architecture::X86_64 => x86_64::runtime_text_stored_suffix_append_width(),
    }
}

pub fn runtime_text_stored_place_append_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_stored_place_append_width(),
        Architecture::X86_64 => x86_64::runtime_text_stored_place_append_width(),
    }
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width(
    architecture: Architecture,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_pointee_width(field_byte_offset)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_append_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_width(literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_append_width(literal),
    }
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(
    architecture: Architecture,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_to_runtime_pointee_width(
            field_byte_offset,
            literal,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_literal_append_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_text_literal_append_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
    }
}

pub fn runtime_text_buffer_materialize_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_width(),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width(
    architecture: Architecture,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_buffer_materialize_to_runtime_pointee_width(field_byte_offset)
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_machine_integer_write_width(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_integer_write_width(byte_offset, byte_size)
        }
        Architecture::X86_64 => x86_64::runtime_machine_integer_write_width(byte_offset, byte_size),
    }
}

pub fn runtime_pointee_integer_write_width(
    architecture: Architecture,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_pointee_integer_write_width(field_byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            x86_64::runtime_pointee_integer_write_width(field_byte_offset, byte_size)
        }
    }
}

/// Bytes inserted between the left and right operand evaluations of a binary
/// write so the left result survives the right evaluation. Zero on aarch64 (it
/// uses distinct result registers); on x86_64 it is a single `push r10`.
pub fn runtime_binary_right_operand_gap(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => x86_64::BINARY_RIGHT_OPERAND_PUSH_WIDTH,
    }
}

pub fn runtime_storage_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::runtime_storage_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

pub fn runtime_pointee_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_binary_write_width(
            runtime_value_operands,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_pointee_operand_start_width(
    architecture: Architecture,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_operand_start_width(field_byte_offset),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_value_compare_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    _byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_compare_width(runtime_value_operands, left, right)
        }
        Architecture::X86_64 => {
            x86_64::runtime_value_compare_width(runtime_value_operands, left, right)
        }
    }
}

pub fn runtime_value_operand_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_operand_width(runtime_value_operands, operand)
        }
        Architecture::X86_64 => {
            x86_64::runtime_value_operand_width(runtime_value_operands, operand)
        }
    }
}

pub fn runtime_frame_indexed_integer_write_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_integer_write_width(
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_indexed_integer_write_width(
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
    }
}

pub fn runtime_frame_base_indexed_integer_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
    }
}

pub fn runtime_frame_base_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_binary_write_width(
            runtime_value_operands,
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_base_indexed_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

/// Byte offset of the left value operand within a frame-base-indexed binary
/// write (i.e. the length of the target-address-computation prefix).
pub fn runtime_frame_base_indexed_binary_left_operand_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            // aarch64 reuses the integer-write prefix length (its store tail is a
            // separate trailing instruction, unlike x86_64's interleaved layout).
            aarch64::runtime_frame_base_indexed_integer_write_width(
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
                0,
            )
        }
        Architecture::X86_64 => {
            let _ = (base_byte_offset, element_byte_size, field_byte_offset);
            x86_64::runtime_frame_base_indexed_binary_left_operand_offset()
        }
    }
}

pub fn runtime_machine_indexed_integer_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_indexed_integer_write_width(
            base_byte_offset,
            index_region,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            x86_64::runtime_machine_indexed_integer_write_width(
                index_region,
                element_byte_size,
                byte_size,
            )
        }
    }
}

pub fn runtime_machine_indexed_integer_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_indexed_integer_runtime_frame_address_offset(base_byte_offset)
        }
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            x86_64::runtime_machine_indexed_integer_runtime_frame_address_offset()
        }
    }
}

pub fn runtime_frame_indexed_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_machine_string_write_width(architecture: Architecture, byte_length: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_string_write_width(byte_length),
        Architecture::X86_64 => x86_64::runtime_machine_string_write_width(byte_length),
    }
}

pub fn runtime_pointee_string_write_width(
    architecture: Architecture,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_pointee_string_write_width(field_byte_offset, byte_length)
        }
        Architecture::X86_64 => {
            x86_64::runtime_pointee_string_write_width(field_byte_offset, byte_length)
        }
    }
}

pub fn runtime_frame_indexed_string_write_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_indexed_string_write_width(
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => x86_64::runtime_frame_indexed_string_write_width(
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
    }
}

pub fn runtime_machine_indexed_string_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_indexed_string_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => x86_64::runtime_machine_indexed_string_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
    }
}

pub fn runtime_machine_indexed_string_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_machine_indexed_string_runtime_frame_address_offset(base_byte_offset)
        }
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            x86_64::MACHINE_INDEXED_STRING_FRAME_IMM_OFFSET
        }
    }
}

pub fn runtime_machine_indexed_string_data_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_machine_indexed_string_data_address_offset(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
        ),
        Architecture::X86_64 => {
            let _ = (base_byte_offset, element_byte_size, field_byte_offset);
            x86_64::MACHINE_INDEXED_STRING_DATA_IMM_OFFSET
        }
    }
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                base_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = base_byte_offset;
            0
        }
    }
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (base_byte_offset, element_byte_size, field_byte_offset);
            0
        }
    }
}

pub fn runtime_storage_address_to_runtime_frame_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_address_to_runtime_frame_write_width(),
        Architecture::X86_64 => x86_64::runtime_storage_address_to_runtime_frame_write_width(),
    }
}

pub fn runtime_pointee_address_to_runtime_frame_write_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_address_to_runtime_frame_write_width(),
        Architecture::X86_64 => x86_64::runtime_pointee_address_to_runtime_frame_write_width(),
    }
}

pub fn runtime_frame_indexed_address_to_runtime_frame_write_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_indexed_address_to_runtime_frame_write_width(
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
                element_index,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (element_index, element_byte_size, field_byte_offset);
            x86_64::runtime_frame_fixed_indexed_address_to_runtime_frame_write_width()
        }
    }
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_base_indexed_address_to_runtime_frame_write_width(
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (base_byte_offset, element_byte_size, field_byte_offset);
            x86_64::runtime_frame_base_indexed_address_to_runtime_frame_write_width()
        }
    }
}

/// Relocation imm offset (pre-`+2`) of the second runtime-frame base load in the
/// frame-base-indexed address write, when the architecture loads the frame base
/// more than once. `None` when a single load is reused (aarch64).
pub fn runtime_frame_base_indexed_address_target_frame_offset(
    architecture: Architecture,
) -> Option<usize> {
    match architecture {
        Architecture::Aarch64 => None,
        Architecture::X86_64 => Some(x86_64::FRAME_BASE_INDEXED_ADDRESS_TARGET_FRAME_IMM_OFFSET),
    }
}

pub fn runtime_text_line_read_width(
    architecture: Architecture,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::runtime_text_line_read_import_width(byte_capacity)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                aarch64::runtime_text_line_read_syscall_width(byte_capacity, *number)
            }
        },
        Architecture::X86_64 => x86_64::runtime_text_line_read_width(byte_capacity),
    }
}

pub fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::runtime_text_line_read_import_target_address_offset()
            }
            HostBindingMechanism::Syscall { number, .. } => {
                aarch64::runtime_text_line_read_syscall_target_address_offset(*number)
            }
        },
        Architecture::X86_64 => x86_64::runtime_text_line_read_target_imm_offset(),
    }
}

pub fn runtime_text_line_read_import_call_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_line_read_import_call_offset(),
        // x86_64 ReadFile call rel32 displacement.
        Architecture::X86_64 => x86_64::runtime_text_line_read_read_file_call_offset(),
    }
}

/// x86_64-only: rel32 displacement offset of the GetStdHandle call within the
/// runtime line-read instruction (aarch64 has no separate handle call).
pub fn runtime_text_line_read_get_std_handle_call_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => x86_64::runtime_text_line_read_get_std_handle_call_offset(),
    }
}

pub fn runtime_storage_copy_width(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_width(source_offset, target_offset, byte_count)
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_width(source_offset, target_offset, byte_count)
        }
    }
}

pub fn runtime_storage_copy_to_runtime_frame_indexed_width(
    architecture: Architecture,
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_to_runtime_frame_indexed_width(
            source_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
        ),
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
        Architecture::X86_64 => x86_64::runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    }
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
    }
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_storage_copy_to_runtime_pointee_width(
    architecture: Architecture,
    source_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_to_runtime_pointee_width(
            source_offset,
            field_byte_offset,
            byte_count,
        ),
        Architecture::X86_64 => x86_64::runtime_storage_copy_to_runtime_pointee_width(
            source_offset,
            field_byte_offset,
            byte_count,
        ),
    }
}
