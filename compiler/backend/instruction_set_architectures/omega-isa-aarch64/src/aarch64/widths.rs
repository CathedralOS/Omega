use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
use omega_core::arena::Arena;
use omega_target_operations::{RuntimeValueOperand, RuntimeValueOperandHandle};

pub fn host_call_sequence_width(operands: &[Aarch64CallOperand]) -> usize {
    operands.iter().map(operand_width).sum::<usize>() + 4
}

pub fn syscall_sequence_width(operands: &[Aarch64CallOperand], syscall_number: u32) -> usize {
    operands.iter().map(operand_width).sum::<usize>()
        + unsigned_immediate_width(u64::from(syscall_number))
        + 4
}

pub fn return_width() -> usize {
    4
}

pub fn dispatch_loop_enter_width() -> usize {
    4
}

pub fn dispatch_case_enter_width() -> usize {
    8
}

pub fn dispatch_state_write_width() -> usize {
    8
}

pub fn dispatch_case_leave_width() -> usize {
    4
}

pub fn dispatch_guard_compare_static_width() -> usize {
    20
}

pub fn runtime_text_literal_compare_width(literal: &str) -> usize {
    8 + literal.len() * 12 + runtime_text_input_delimiter_check_width()
}

pub fn runtime_text_storage_compare_width() -> usize {
    84
}

pub fn runtime_storage_compare_width() -> usize {
    32
}

pub fn runtime_storage_value_compare_width() -> usize {
    20
}

fn runtime_text_input_delimiter_check_width() -> usize {
    32
}

pub fn runtime_text_literal_write_width(literal: &str) -> usize {
    8 + literal.len() * 8
}

pub fn runtime_text_literal_segment_write_width(literal: &str) -> usize {
    runtime_text_literal_write_width(literal)
}

pub fn runtime_text_stored_suffix_append_width() -> usize {
    72
}

pub fn runtime_text_stored_place_append_width() -> usize {
    80
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 72
}

pub fn runtime_text_literal_append_width(literal: &str) -> usize {
    40 + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 32 + literal.len() * 8
}

pub fn runtime_text_buffer_materialize_width() -> usize {
    60
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 48
}

pub fn runtime_machine_integer_write_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 => 16,
        8 => 28,
        _ => 0,
    }
}

pub fn runtime_pointee_integer_write_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 => 20,
        8 => 32,
        _ => 0,
    }
}

pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    8 + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(byte_size)
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    12 + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(byte_size)
}

pub fn runtime_frame_indexed_integer_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + runtime_store_data_width(byte_size)
}

pub fn runtime_frame_indexed_binary_write_width(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(byte_size)
}

pub fn runtime_machine_string_write_width(byte_length: usize) -> usize {
    24 + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_pointee_string_write_width(byte_length: usize) -> usize {
    28 + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_text_line_read_import_width(_byte_capacity: usize) -> usize {
    100
}

pub fn runtime_text_line_read_syscall_width(_byte_capacity: usize, syscall_number: u32) -> usize {
    100 + unsigned_immediate_width(u64::from(syscall_number))
}

pub fn runtime_text_line_read_import_target_address_offset() -> usize {
    84
}

pub fn runtime_text_line_read_syscall_target_address_offset(syscall_number: u32) -> usize {
    84 + unsigned_immediate_width(u64::from(syscall_number))
}

pub fn runtime_text_line_read_import_call_offset() -> usize {
    28
}

pub fn runtime_storage_copy_width(byte_count: usize) -> usize {
    16 + runtime_storage_copy_data_width(byte_count)
}

pub fn runtime_storage_copy_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 8
        + runtime_storage_copy_data_width(byte_count)
}

pub fn runtime_storage_copy_to_runtime_pointee_width(
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    20 + add_constant_width(field_byte_offset) + runtime_storage_copy_data_width(byte_count)
}

pub fn operand_width(operand: &Aarch64CallOperand) -> usize {
    match operand {
        DataAddress { .. } => 8,
        RuntimeStringPointer { .. } | RuntimeStringLength { .. } => 12,
        ImmediateInteger(value) => immediate_width(*value),
        ByteLength(value) => unsigned_immediate_width(*value as u64),
    }
}

fn immediate_width(value: i64) -> usize {
    match u64::try_from(value) {
        Ok(value) => unsigned_immediate_width(value),
        Err(_) => 4,
    }
}

fn unsigned_immediate_width(value: u64) -> usize {
    let high_nonzero_halfwords = (1..4)
        .filter(|halfword_shift| halfword(value, *halfword_shift) != 0)
        .count();

    4 + high_nonzero_halfwords * 4
}

fn runtime_storage_copy_data_width(byte_count: usize) -> usize {
    match byte_count {
        1 | 4 => 8,
        _ if byte_count.is_multiple_of(8) => (byte_count / 8) * 8,
        _ => 0,
    }
}

fn runtime_value_operand_width(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    operand: RuntimeValueOperandHandle,
) -> usize {
    match runtime_value_operands.get(operand) {
        RuntimeValueOperand::Immediate(value) => immediate_width(*value),
        RuntimeValueOperand::Storage { byte_size, .. } => match byte_size {
            1 | 4 => 12,
            8 => 20,
            _ => 0,
        },
        RuntimeValueOperand::Pointee {
            field_byte_offset,
            byte_size,
            ..
        } => {
            20 + add_constant_width(*field_byte_offset)
                + runtime_store_data_width(*byte_size)
        }
        RuntimeValueOperand::FrameIndexed {
            element_byte_size,
            field_byte_offset,
            byte_size,
            ..
        } => {
            runtime_frame_index_setup_width(*element_byte_size, *field_byte_offset)
                + runtime_store_data_width(*byte_size)
        }
        RuntimeValueOperand::Binary { left, right, .. } => {
            runtime_value_operand_width(runtime_value_operands, *left)
                + runtime_value_operand_width(runtime_value_operands, *right)
                + 4
        }
    }
}

fn runtime_binary_operation_width(byte_size: usize) -> usize {
    16 + runtime_store_data_width(byte_size)
}

fn runtime_store_data_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 12,
        _ => 0,
    }
}

fn runtime_frame_index_setup_width(element_byte_size: usize, field_byte_offset: usize) -> usize {
    12 + 12 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset)
}

fn scale_index_width(element_byte_size: usize) -> usize {
    if element_byte_size == 0 {
        return 0;
    }

    let highest_bit = usize::BITS - element_byte_size.leading_zeros();
    let doubles = highest_bit.saturating_sub(1) as usize;
    let additions = element_byte_size.count_ones() as usize;
    8 + (doubles + additions) * 4
}

fn add_constant_width(value: usize) -> usize {
    if value == 0 {
        0
    } else if value <= 4095 {
        4
    } else {
        unsigned_immediate_width(value as u64) + 4
    }
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
