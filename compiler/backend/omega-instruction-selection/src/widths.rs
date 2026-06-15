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
        Architecture::X86_64 => x86_64::syscall_sequence_width(operands),
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

pub fn runtime_storage_copy_to_return_register_width(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_to_return_register_width(byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_to_return_register_width(byte_offset, byte_size)
        }
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

pub fn dispatch_guard_compare_static_width(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::dispatch_guard_compare_static_width(byte_offset, byte_size, is_float)
        }
        Architecture::X86_64 => x86_64::dispatch_guard_compare_static_width(is_float, byte_size),
    }
}

pub fn runtime_text_literal_compare_width(architecture: Architecture, literal: &str) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_compare_width(literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_compare_width(literal),
    }
}

pub fn runtime_text_storage_compare_width(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            let _ = literal_len;
            aarch64::runtime_text_storage_compare_width(source_offset)
        }
        Architecture::X86_64 => x86_64::runtime_text_storage_compare_width_x86(literal_len),
    }
}

/// Byte offset within a `CompareRuntimeTextStorage` of the failure branch the
/// emitter must target with the compare-failure distance.
pub fn runtime_text_storage_compare_failure_branch_offset(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
) -> usize {
    match architecture {
        // AArch64's per-byte mismatch conditional branch follows the two descriptor loads.
        Architecture::Aarch64 => {
            let _ = literal_len;
            16 + aarch64_runtime_text_descriptor_load_pair_width(source_offset) + 16
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
    source_offset: usize,
    literal_len: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_storage_compare_width(source_offset).saturating_sub(4)
        }
        Architecture::X86_64 => {
            x86_64::runtime_text_storage_compare_failure_branch_offset(literal_len)
        }
    }
}

fn aarch64_runtime_text_descriptor_load_pair_width(byte_offset: usize) -> usize {
    aarch64_data_offset_load_width(byte_offset, 8)
        + aarch64_data_offset_load_width(byte_offset + 8, 8)
}

fn aarch64_data_offset_load_width(byte_offset: usize, byte_size: usize) -> usize {
    if aarch64_data_offset_encodable(byte_offset, byte_size) {
        4
    } else {
        4 + aarch64_add_constant_width(byte_offset) + 4
    }
}

fn aarch64_data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}

fn aarch64_add_constant_width(value: usize) -> usize {
    if value == 0 {
        0
    } else if value <= 4095 {
        4
    } else {
        aarch64_unsigned_immediate_width(value as u64) + 4
    }
}

fn aarch64_unsigned_immediate_width(value: u64) -> usize {
    let high_nonzero_halfwords = (1..4)
        .filter(|halfword_shift| ((value >> (halfword_shift * 16)) & 0xffff) != 0)
        .count();

    4 + high_nonzero_halfwords * 4
}

pub fn runtime_storage_compare_width(
    architecture: Architecture,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_compare_width(
            left_offset,
            right_offset,
            byte_size,
            is_float,
        ),
        Architecture::X86_64 => omega_isa_x86_64::runtime_storage_compare_width(
            left_offset,
            right_offset,
            byte_size,
            is_float,
        ),
    }
}

pub fn runtime_storage_value_compare_width(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_value_compare_width(byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            omega_isa_x86_64::runtime_storage_value_compare_width(byte_offset, byte_size)
        }
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

pub fn runtime_text_stored_suffix_append_width(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_stored_suffix_append_width(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
        Architecture::X86_64 => x86_64::runtime_text_stored_suffix_append_width(),
    }
}

pub fn runtime_text_stored_place_append_width(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_width(source_offset, target_offset)
        }
        Architecture::X86_64 => x86_64::runtime_text_stored_place_append_width(),
    }
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    architecture: Architecture,
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_width(
                source_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width(
    architecture: Architecture,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_text_stored_place_append_to_runtime_pointee_width(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (source_offset, pointer_byte_offset, field_byte_offset);
            x86_64::runtime_text_stored_place_append_to_runtime_pointee_width()
        }
    }
}

pub fn runtime_text_literal_append_width(
    architecture: Architecture,
    target_offset: usize,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_width(target_offset, literal),
        Architecture::X86_64 => x86_64::runtime_text_literal_append_width(literal),
    }
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_literal_append_to_runtime_pointee_width(
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
        Architecture::X86_64 => {
            let _ = (pointer_byte_offset, field_byte_offset);
            x86_64::runtime_text_literal_append_to_runtime_pointee_width(literal)
        }
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
        Architecture::X86_64 => x86_64::runtime_text_literal_append_to_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            literal,
        ),
    }
}

pub fn runtime_text_buffer_materialize_width(
    architecture: Architecture,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_width(target_offset),
        Architecture::X86_64 => {
            let _ = target_offset;
            x86_64::runtime_text_buffer_materialize_width()
        }
    }
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_text_buffer_materialize_to_runtime_pointee_width(
            pointer_byte_offset,
            field_byte_offset,
        ),
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
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_integer_write_width(
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        ),
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
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

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: omega_core::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> usize {
    match architecture {
        // aarch64 does not implement the saturating/trapping domains yet (the
        // emitter errors); its width is unaffected by the domain.
        Architecture::Aarch64 => aarch64::runtime_storage_binary_write_width(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
        ),
        Architecture::X86_64 => x86_64::runtime_storage_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_convert_width(
            runtime_value_operands,
            target_offset,
            source,
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        ),
        Architecture::X86_64 => {
            // The x86_64 converting store reaches the target through `r14`-relative
            // addressing with a fixed-width displacement, so its width is
            // offset-independent; ignore `target_offset` here.
            let _ = target_offset;
            x86_64::runtime_storage_convert_width(
                runtime_value_operands,
                source,
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
            )
        }
    }
}

pub fn runtime_pointee_binary_write_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_binary_write_width(
            runtime_value_operands,
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => {
            let _ = (pointer_byte_offset, field_byte_offset);
            x86_64::runtime_pointee_binary_write_width(
                runtime_value_operands,
                byte_size,
                left,
                operator,
                right,
            )
        }
    }
}

pub fn runtime_pointee_operand_start_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_pointee_operand_start_width(pointer_byte_offset, field_byte_offset)
        }
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
            let _ = field_byte_offset;
            x86_64::runtime_pointee_binary_operand_start_width()
        }
    }
}

pub fn runtime_value_compare_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_value_compare_width(runtime_value_operands, left, right)
        }
        Architecture::X86_64 => {
            x86_64::runtime_value_compare_width(runtime_value_operands, byte_size, left, right)
        }
    }
}

/// Byte offset of the RIGHT descriptor base materialization inside a
/// `TextEquals` value operand (the left base sits at the operand start). The
/// relocation planner pins both region symbols through these offsets.
pub fn runtime_text_equals_right_base_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
        Architecture::X86_64 => x86_64::RUNTIME_TEXT_EQUALS_RIGHT_BASE_OFFSET,
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
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            index_offset,
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
    index_offset: usize,
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
            index_offset,
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
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            // aarch64 reuses the integer-write prefix length (its store tail is a
            // separate trailing instruction, unlike x86_64's interleaved layout).
            aarch64::runtime_frame_base_indexed_integer_write_width(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                0,
            )
        }
        Architecture::X86_64 => {
            let _ = (
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            );
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

pub fn runtime_frame_string_write_width(architecture: Architecture, byte_length: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_frame_string_write_width(byte_length),
        Architecture::X86_64 => x86_64::runtime_frame_string_write_width(byte_length),
    }
}

pub fn runtime_pointee_string_write_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_string_write_width(
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
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

pub fn runtime_storage_address_to_runtime_frame_write_width(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_address_to_runtime_frame_write_width(
            source_offset,
            target_offset,
        ),
        Architecture::X86_64 => x86_64::runtime_storage_address_to_runtime_frame_write_width(),
    }
}

pub fn runtime_storage_address_to_runtime_frame_target_frame_offset(
    architecture: Architecture,
    source_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_address_to_runtime_frame_target_frame_offset(source_offset)
        }
        Architecture::X86_64 => {
            let _ = source_offset;
            17
        }
    }
}

pub fn runtime_pointee_address_to_runtime_frame_write_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_pointee_address_to_runtime_frame_write_width(
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
        ),
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
            x86_64::runtime_pointee_address_to_runtime_frame_write_width()
        }
    }
}

pub fn runtime_frame_indexed_address_to_runtime_frame_write_width(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_indexed_address_to_runtime_frame_write_width(
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => 0,
    }
}

pub fn runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
    architecture: Architecture,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
            );
            x86_64::runtime_frame_fixed_indexed_address_to_runtime_frame_write_width()
        }
    }
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_width(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_frame_base_indexed_address_to_runtime_frame_write_width(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = (
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            );
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
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::runtime_text_line_read_width(byte_capacity)
            }
            HostBindingMechanism::Syscall { .. } => x86_64::runtime_text_line_read_syscall_width(),
        },
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
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::runtime_text_line_read_target_imm_offset()
            }
            HostBindingMechanism::Syscall { .. } => {
                x86_64::runtime_text_line_read_syscall_target_imm_offset()
            }
        },
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

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
                element_index,
                element_byte_size,
                source_field_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
                element_index,
                element_byte_size,
                source_field_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
    }
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
    architecture: Architecture,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
                element_byte_size,
                source_field_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
                element_byte_size,
                source_field_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
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
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::runtime_storage_copy_to_runtime_pointee_width(
            source_offset,
            pointer_byte_offset,
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

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            let _ = pointer_byte_offset;
            x86_64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
    }
}

pub fn append_wire_literal_byte_width(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_literal_byte_width(out_offset, written_offset),
        Architecture::X86_64 => x86_64::append_wire_literal_byte_width(out_offset, written_offset),
    }
}

pub fn append_wire_scalar_varint_width(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            out_offset,
            written_offset,
        ),
    }
}

pub fn append_wire_text_bytes_width(
    architecture: Architecture,
    source_offset: usize,
    out_offset: usize,
    out_length: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_text_bytes_width(
            source_offset,
            out_offset,
            out_length,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_text_bytes_width(
            source_offset,
            out_offset,
            out_length,
            written_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn append_wire_repeated_scalar_varint_width(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    zigzag: bool,
    index: u64,
    count_offset: usize,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::append_wire_repeated_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset,
        ),
        Architecture::X86_64 => x86_64::append_wire_repeated_scalar_varint_width(
            source_offset,
            byte_size,
            zigzag,
            index,
            count_offset,
            out_offset,
            written_offset,
        ),
    }
}

/// Byte offset of the COUNT page address materialization inside the repeated
/// append (relocated to the count companion's region symbol).
pub fn wire_append_repeated_count_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::wire_append_repeated_count_page_offset(out_offset, written_offset)
        }
        Architecture::X86_64 => {
            x86_64::wire_append_repeated_count_page_offset(out_offset, written_offset)
        }
    }
}

/// Byte offset of the SOURCE page address materialization inside the repeated
/// append (relocated to the element's region symbol).
pub fn wire_append_repeated_source_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
    count_offset: usize,
    index: u64,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_append_repeated_source_page_offset(
            out_offset,
            written_offset,
            count_offset,
            index,
        ),
        Architecture::X86_64 => x86_64::wire_append_repeated_source_page_offset(
            out_offset,
            written_offset,
            count_offset,
            index,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_repeated_scalar_varint_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
    count_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_repeated_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_offset,
            target_offset,
            byte_size,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::read_wire_repeated_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
            count_offset,
            target_offset,
            byte_size,
            zigzag,
        ),
    }
}

/// Byte offset of the TARGET page address materialization inside the repeated
/// read (relocated to the element slot's region symbol).
pub fn wire_decode_repeated_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_repeated_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::wire_decode_repeated_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            zigzag,
        ),
    }
}

/// Byte offset of the COUNT page address materialization inside the repeated
/// read (relocated to the count companion's region symbol).
#[allow(clippy::too_many_arguments)]
pub fn wire_decode_repeated_count_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    end_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_repeated_count_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            target_offset,
            byte_size,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::wire_decode_repeated_count_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            end_offset,
            target_offset,
            byte_size,
            zigzag,
        ),
    }
}

/// Byte offset of the WRITTEN page address materialization inside both wire
/// appends (relocated to the written slot's region symbol).
pub fn wire_append_written_page_offset(architecture: Architecture, out_offset: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_append_written_page_offset(out_offset),
        Architecture::X86_64 => x86_64::wire_append_written_page_offset(out_offset),
    }
}

/// Byte offset of the SOURCE page address materialization inside the varint
/// append (relocated to the scalar's region symbol).
pub fn wire_append_varint_source_page_offset(
    architecture: Architecture,
    out_offset: usize,
    written_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::wire_append_varint_source_page_offset(out_offset, written_offset)
        }
        Architecture::X86_64 => {
            x86_64::wire_append_varint_source_page_offset(out_offset, written_offset)
        }
    }
}

pub fn read_wire_expected_byte_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_expected_byte_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
        ),
        Architecture::X86_64 => x86_64::read_wire_expected_byte_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn read_wire_scalar_varint_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    target_offset: usize,
    byte_size: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::read_wire_scalar_varint_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            target_offset,
            byte_size,
            zigzag,
        ),
    }
}

/// Byte offset of the READ (cursor) page address materialization inside both
/// wire decodes (relocated to the read slot's region symbol).
pub fn wire_decode_read_page_offset(architecture: Architecture, buffer_offset: usize) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_read_page_offset(buffer_offset),
        Architecture::X86_64 => x86_64::wire_decode_read_page_offset(buffer_offset),
    }
}

/// Byte offset of the OK (sticky flag) page address materialization inside
/// both wire decodes (relocated to the ok slot's region symbol).
pub fn wire_decode_ok_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_ok_page_offset(buffer_offset, read_offset),
        Architecture::X86_64 => x86_64::wire_decode_ok_page_offset(buffer_offset, read_offset),
    }
}

/// Byte offset of the TARGET page address materialization inside the varint
/// decode (relocated to the field's region symbol).
pub fn wire_decode_varint_target_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    zigzag: bool,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::wire_decode_varint_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            zigzag,
        ),
        Architecture::X86_64 => x86_64::wire_decode_varint_target_page_offset(
            buffer_offset,
            buffer_length,
            read_offset,
            zigzag,
        ),
    }
}

pub fn read_wire_nested_open_width(
    architecture: Architecture,
    buffer_offset: usize,
    buffer_length: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::read_wire_nested_open_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
        ),
        Architecture::X86_64 => x86_64::read_wire_nested_open_width(
            buffer_offset,
            buffer_length,
            read_offset,
            ok_offset,
            end_offset,
        ),
    }
}

pub fn read_wire_nested_close_width(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
    ok_offset: usize,
    end_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
        }
        Architecture::X86_64 => {
            x86_64::read_wire_nested_close_width(buffer_offset, read_offset, ok_offset, end_offset)
        }
    }
}

/// Byte offset of the END-slot page address materialization inside both
/// nested decodes (relocated to the end slot's region symbol).
pub fn wire_decode_nested_end_page_offset(
    architecture: Architecture,
    buffer_offset: usize,
    read_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::wire_decode_nested_end_page_offset(buffer_offset, read_offset)
        }
        Architecture::X86_64 => {
            x86_64::wire_decode_nested_end_page_offset(buffer_offset, read_offset)
        }
    }
}
