use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};

pub fn host_call_sequence_width(operands: &[Aarch64CallOperand]) -> usize {
    host_call_sequence_width_from_operands(operands.iter().copied())
}

pub fn syscall_sequence_width(operands: &[Aarch64CallOperand], syscall_number: u32) -> usize {
    syscall_sequence_width_from_operands(operands.iter().copied(), syscall_number)
}

pub fn host_call_sequence_width_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand>,
) -> usize {
    operands
        .map(|operand| operand_width(&operand))
        .sum::<usize>()
        + 4
}

pub fn syscall_sequence_width_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand>,
    syscall_number: u32,
) -> usize {
    operands
        .map(|operand| operand_width(&operand))
        .sum::<usize>()
        + unsigned_immediate_width(u64::from(syscall_number))
        + 4
}

pub fn function_enter_width() -> usize {
    28
}

pub fn return_width() -> usize {
    28
}

pub fn return_register_integer_write_width() -> usize {
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

pub fn runtime_value_compare_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + 8
}

pub(in crate::aarch64) fn runtime_text_input_delimiter_check_width() -> usize {
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
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 76
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width(
    field_byte_offset: usize,
) -> usize {
    84 + add_constant_width(field_byte_offset)
}

pub fn runtime_text_literal_append_width(literal: &str) -> usize {
    36 + add_constant_width(literal.len()) + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    40 + add_constant_width(field_byte_offset)
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 28
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_buffer_materialize_width() -> usize {
    60
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 52
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width(field_byte_offset: usize) -> usize {
    64 + add_constant_width(field_byte_offset)
}

pub fn runtime_machine_integer_write_width(byte_offset: usize, byte_size: usize) -> usize {
    8 + add_constant_width(byte_offset) + runtime_store_data_width(byte_size)
}

pub fn runtime_pointee_integer_write_width(field_byte_offset: usize, byte_size: usize) -> usize {
    let width = match byte_size {
        1 | 4 => 20,
        8 => 32,
        _ => 0,
    };

    width + add_constant_width(field_byte_offset)
}

pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    8 + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + runtime_result_write_width(byte_size)
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    12 + add_constant_width(field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + runtime_result_write_width(byte_size)
}

pub fn runtime_pointee_operand_start_width(field_byte_offset: usize) -> usize {
    12 + add_constant_width(field_byte_offset)
}

pub fn runtime_frame_indexed_integer_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + runtime_store_data_width(byte_size)
}

pub fn runtime_frame_base_indexed_integer_write_width(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    20 + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + runtime_store_data_width(byte_size)
}

pub fn runtime_machine_indexed_integer_write_width(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            28 + add_constant_width(base_byte_offset)
                + scale_index_width(element_byte_size)
                + add_constant_width(field_byte_offset)
                + runtime_store_data_width(byte_size)
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            20 + add_constant_width(base_byte_offset)
                + scale_index_width(element_byte_size)
                + add_constant_width(field_byte_offset)
                + runtime_store_data_width(byte_size)
        }
    }
}

pub fn runtime_machine_indexed_integer_runtime_frame_address_offset(
    base_byte_offset: usize,
) -> usize {
    12 + add_constant_width(base_byte_offset)
}

pub fn runtime_frame_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + runtime_result_write_width(byte_size)
}

pub fn runtime_frame_base_indexed_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    20 + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + runtime_result_write_width(byte_size)
}

pub fn runtime_machine_string_write_width(byte_length: usize) -> usize {
    24 + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_pointee_string_write_width(field_byte_offset: usize, byte_length: usize) -> usize {
    28 + add_constant_width(field_byte_offset) + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_frame_indexed_string_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 8
        + 4
        + unsigned_immediate_width(byte_length as u64)
        + 4
}

pub fn runtime_machine_indexed_string_write_width(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    28 + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + 8
        + 4
        + unsigned_immediate_width(byte_length as u64)
        + 4
}

pub fn runtime_machine_indexed_string_runtime_frame_address_offset(
    base_byte_offset: usize,
) -> usize {
    20 + add_constant_width(base_byte_offset)
}

pub fn runtime_machine_indexed_string_data_address_offset(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    28 + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
    base_byte_offset: usize,
) -> usize {
    12 + add_constant_width(base_byte_offset)
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    28 + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
}

pub fn runtime_storage_address_to_runtime_frame_write_width() -> usize {
    24
}

pub fn runtime_pointee_address_to_runtime_frame_write_width() -> usize {
    20
}

pub fn runtime_frame_indexed_address_to_runtime_frame_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    24 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset)
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

pub fn runtime_storage_copy_width(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    16 + add_constant_width(source_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_to_runtime_frame_indexed_width(
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + add_constant_width(source_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 8
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(field_byte_offset);
    12 + add_constant_width(source_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(field_byte_offset);
    20 + add_constant_width(source_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    32 + add_constant_width(base_byte_offset)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_to_runtime_pointee_width(
    source_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    20 + add_constant_width(field_byte_offset)
        + add_constant_width(source_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
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

fn runtime_storage_copy_data_width(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
) -> usize {
    let mut remaining = byte_count;
    let mut offset = 0usize;
    let mut width = 0usize;

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size =
            if remaining >= 8 && source_offset.is_multiple_of(8) && target_offset.is_multiple_of(8)
            {
                8
            } else if remaining >= 4
                && source_offset.is_multiple_of(4)
                && target_offset.is_multiple_of(4)
            {
                4
            } else {
                1
            };

        width += 8;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    width
}

pub fn runtime_value_operand_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    operand: RuntimeValueOperandHandle,
) -> usize {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        immediate_width(value)
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        8 + add_constant_width(byte_offset) + runtime_load_data_width(byte_size)
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        12 + add_constant_width(pointer_byte_offset)
            + add_constant_width(field_byte_offset)
            + runtime_load_data_width(byte_size)
    } else if let Some((_, _, element_byte_size, field_byte_offset, byte_size)) =
        runtime_value_operands.frame_indexed(operand)
    {
        runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
            + runtime_load_data_width(byte_size)
    } else if let Some((base_byte_offset, _, element_byte_size, field_byte_offset, byte_size)) =
        runtime_value_operands.frame_base_indexed(operand)
    {
        runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ) - runtime_store_data_width(byte_size)
            + runtime_load_data_width(byte_size)
    } else if let Some((_, _, element_byte_size, field_byte_offset, byte_size)) =
        runtime_value_operands.frame_fixed_indexed(operand)
    {
        runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
            + runtime_load_data_width(byte_size)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + runtime_binary_operation_width(operator)
    } else {
        0
    }
}

fn runtime_binary_operation_width(operator: StateGuardOperator) -> usize {
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::Subtract
        | StateGuardOperator::Multiply => 4,
        StateGuardOperator::Modulo => 8,
        StateGuardOperator::Max | StateGuardOperator::Min => 12,
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual => 16,
        _ => 0,
    }
}

fn runtime_store_data_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 => 8,
        8 => 20,
        _ => 0,
    }
}

fn runtime_load_data_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 4,
        _ => 0,
    }
}

fn runtime_result_write_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 4 | 8 => 4,
        _ => 0,
    }
}

pub(in crate::aarch64) fn runtime_frame_index_setup_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    20 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset)
}

pub(in crate::aarch64) fn scale_index_width(element_byte_size: usize) -> usize {
    if element_byte_size == 0 {
        return 0;
    }

    let highest_bit = usize::BITS - element_byte_size.leading_zeros();
    let doubles = highest_bit.saturating_sub(1) as usize;
    let additions = element_byte_size.count_ones() as usize;
    8 + (doubles + additions) * 4
}

pub(in crate::aarch64) fn add_constant_width(value: usize) -> usize {
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
