use crate::instructions::{InstructionOperand, InstructionOperandKind};

pub fn host_call_sequence_width(operands: &[InstructionOperand]) -> usize {
    operands.iter().map(operand_width).sum::<usize>() + 4
}

pub fn syscall_sequence_width(operands: &[InstructionOperand], syscall_number: u32) -> usize {
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

pub fn runtime_text_literal_append_width(literal: &str) -> usize {
    40 + literal.len() * 8
}

pub fn runtime_text_buffer_materialize_width() -> usize {
    60
}

pub fn runtime_machine_integer_write_width() -> usize {
    16
}

pub fn runtime_machine_string_write_width(byte_length: usize) -> usize {
    24 + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_text_line_read_width(_byte_capacity: usize, syscall_number: u32) -> usize {
    100 + unsigned_immediate_width(u64::from(syscall_number))
}

pub fn runtime_text_line_read_target_address_offset(syscall_number: u32) -> usize {
    84 + unsigned_immediate_width(u64::from(syscall_number))
}

pub fn runtime_storage_copy_width(byte_count: usize) -> usize {
    16 + runtime_storage_copy_data_width(byte_count)
}

pub fn operand_width(operand: &InstructionOperand) -> usize {
    match &operand.kind {
        InstructionOperandKind::DataAddress { .. } => 8,
        InstructionOperandKind::RuntimeMachineStringPointer { .. }
        | InstructionOperandKind::RuntimeMachineStringLength { .. } => 12,
        InstructionOperandKind::ImmediateInteger(value) => immediate_width(*value),
        InstructionOperandKind::ByteLength(value) => unsigned_immediate_width(*value as u64),
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

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
