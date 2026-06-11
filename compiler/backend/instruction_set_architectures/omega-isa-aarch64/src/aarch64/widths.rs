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

pub fn runtime_storage_copy_to_return_register_width(byte_offset: usize, byte_size: usize) -> usize {
    // adrp+add (8) + scalar load into w0/x0 + sign extension for narrow operands
    // (SXTB/SXTH, 4) so a negative i8/i16 terminal survives the widening read.
    let extend_width = if matches!(byte_size, 1 | 2) { 4 } else { 0 };
    8 + load_data_offset_width(byte_offset, byte_size) + extend_width
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

pub fn dispatch_guard_compare_static_width(
    byte_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    // adrp+add (8) + guard load + [SXTB/SXTH for narrow operands (4)] + expected
    // materialization (padded W = 8, padded X = 16) + [2 FMOVs for floats (8)]
    // + compare (CMP or FCMP, 4) + conditional branch (4).
    let extend_width = if !is_float && matches!(byte_size, 1 | 2) {
        4
    } else {
        0
    };
    let materialize_width = if byte_size == 8 { 16 } else { 8 };
    let float_move_width = if is_float { 8 } else { 0 };
    16 + extend_width
        + materialize_width
        + float_move_width
        + load_data_offset_width(byte_offset, byte_size)
}

pub fn runtime_text_literal_compare_width(literal: &str) -> usize {
    8 + literal.len() * 12 + runtime_text_input_delimiter_check_width()
}

pub fn runtime_text_storage_compare_width(source_offset: usize) -> usize {
    76 + runtime_text_descriptor_load_pair_width(source_offset)
}

pub fn runtime_storage_compare_width(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    is_float: bool,
) -> usize {
    // Float adds two `FMOV` (GPR -> FP) instructions (8 bytes) before the FCMP.
    // 2-byte integer operands add two `SXTH` instructions before the compare.
    let float_move_width = if is_float { 8 } else { 0 };
    let extend_width = if !is_float && byte_size == 2 { 8 } else { 0 };
    24 + float_move_width
        + extend_width
        + load_data_offset_width(left_offset, byte_size)
        + load_data_offset_width(right_offset, byte_size)
}

pub fn runtime_storage_value_compare_width(byte_offset: usize, byte_size: usize) -> usize {
    // adrp+add (8) + load + [SXTB/SXTH for narrow operands (4)] + expected
    // materialization (padded W = 8, padded X = 16) + compare (4) + branch (4).
    let extend_width = if matches!(byte_size, 1 | 2) { 4 } else { 0 };
    let materialize_width = if byte_size == 8 { 16 } else { 8 };
    16 + extend_width + materialize_width + load_data_offset_width(byte_offset, byte_size)
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

pub fn runtime_text_stored_suffix_append_width(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> usize {
    48 + runtime_text_descriptor_load_pair_width(source_offset)
        + add_constant_width(buffer_offset)
        + runtime_text_descriptor_store_pair_width(target_offset)
        + add_constant_width(length_delta)
}

pub fn runtime_text_stored_place_append_width(source_offset: usize, target_offset: usize) -> usize {
    60 + load_data_offset_width(target_offset + 8, 8)
        + runtime_text_descriptor_load_pair_width(source_offset)
        + runtime_text_descriptor_store_pair_width(target_offset)
}

pub fn runtime_text_stored_place_append_to_runtime_frame_indexed_width(
    source_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        + 64
        + runtime_text_descriptor_load_pair_width(source_offset)
}

pub fn runtime_text_stored_place_append_to_runtime_pointee_width(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    60 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + load_data_offset_width(8, 8)
        + runtime_text_descriptor_load_pair_width(source_offset)
        + runtime_text_descriptor_store_pair_width(0)
}

pub fn runtime_text_literal_append_width(target_offset: usize, literal: &str) -> usize {
    24 + load_data_offset_width(target_offset + 8, 8)
        + runtime_text_descriptor_store_pair_width(target_offset)
        + add_constant_width(literal.len())
        + literal.len() * 8
}

pub fn runtime_text_literal_append_to_runtime_pointee_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> usize {
    24 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + load_data_offset_width(8, 8)
        + runtime_text_descriptor_store_pair_width(0)
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

pub fn runtime_text_buffer_materialize_width(target_offset: usize) -> usize {
    44 + runtime_text_descriptor_load_pair_width(target_offset)
        + runtime_text_descriptor_store_pair_width(target_offset)
}

pub fn runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 52
}

pub fn runtime_text_buffer_materialize_to_runtime_pointee_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    40 + load_data_offset_width(pointer_byte_offset, 8)
        + add_constant_width(field_byte_offset)
        + runtime_text_descriptor_load_pair_width(0)
        + runtime_text_descriptor_store_pair_width(0)
}

pub fn runtime_machine_integer_write_width(byte_offset: usize, byte_size: usize) -> usize {
    8 + add_constant_width(byte_offset) + runtime_store_data_width(byte_size)
}

pub fn runtime_pointee_integer_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    // adrp+add (8) + pointer load (4) + value materialization (padded W = 8,
    // padded X = 16) + sized store (4).
    let width = match byte_size {
        1 | 2 | 4 => 24,
        8 => 32,
        _ => 0,
    };

    width + add_constant_width(pointer_byte_offset) + add_constant_width(field_byte_offset)
}

#[allow(clippy::too_many_arguments)]
pub fn runtime_storage_convert_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> usize {
    // `adrp x16 + add x16` (8) — target base, held across source evaluation —
    // then load the source into x17, convert it in place, and store the result.
    8 + runtime_value_operand_width(runtime_value_operands, source)
        + runtime_convert_operation_width(
            source_byte_size,
            target_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        )
        + runtime_result_write_width(target_offset, target_byte_size)
}

/// Width of the in-register conversion sequence (see
/// `runtime_storage::append_runtime_convert_operation`). The source bits start in
/// x17 and the converted result is left in x17.
fn runtime_convert_operation_width(
    source_byte_size: usize,
    target_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> usize {
    match (source_is_float, target_is_float) {
        // int -> float: SCVTF (4) + FMOV result back to GPR (4).
        (false, true) => 8,
        // float -> int: FMOV bits into FP bank (4) + FCVTZS (4).
        (true, false) => 8,
        (true, true) => {
            if source_byte_size == target_byte_size {
                0 // same precision: bits already in x17.
            } else {
                // FMOV into FP bank (4) + FCVT precision change (4) + FMOV back (4).
                12
            }
        }
        (false, false) => {
            // Sign-extend a narrow signed source when widening; otherwise the load
            // already zero-extended and the store truncates. SXTW (4) handles the
            // signed 32->64 widening case.
            if target_byte_size > source_byte_size && source_signed && source_byte_size == 4 {
                4 // SXTW x17, w17
            } else {
                0
            }
        }
    }
}

pub fn runtime_storage_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
) -> usize {
    let indexed_operand_restore_width = if runtime_value_operands.frame_indexed(left).is_some()
        || runtime_value_operands.frame_indexed(right).is_some()
        || runtime_value_operands.frame_base_indexed(left).is_some()
        || runtime_value_operands.frame_base_indexed(right).is_some()
    {
        4
    } else {
        0
    };

    let operation_width = if is_float {
        runtime_float_binary_operation_width()
    } else {
        runtime_binary_operation_width(operator)
    };

    8 + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + operation_width
        + indexed_operand_restore_width
        + runtime_result_write_width(target_offset, byte_size)
}

/// Width of the float binary-operation sequence: two `FMOV` from GPR (4 bytes
/// each), the single scalar FP op (4), and one `FMOV` back to a GPR (4).
fn runtime_float_binary_operation_width() -> usize {
    16
}

pub fn runtime_pointee_binary_write_width(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> usize {
    12 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + runtime_result_write_width(0, byte_size)
}

pub fn runtime_pointee_operand_start_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    8 + add_constant_width(pointer_byte_offset)
        + runtime_load_data_width(8)
        + add_constant_width(field_byte_offset)
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
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
) -> usize {
    16 + add_constant_width(base_byte_offset)
        // Index is loaded as a 32-bit (4-byte) value, see
        // append_runtime_frame_base_index_target_address.
        + load_data_offset_width(index_offset, 4)
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
        + runtime_result_write_width(0, byte_size)
}

pub fn runtime_frame_base_indexed_binary_write_width(
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
    16 + add_constant_width(base_byte_offset)
        // Index is loaded as a 32-bit (4-byte) value, see
        // append_runtime_frame_base_index_target_address.
        + load_data_offset_width(index_offset, 4)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + runtime_value_operand_width(runtime_value_operands, left)
        + runtime_value_operand_width(runtime_value_operands, right)
        + runtime_binary_operation_width(operator)
        + runtime_result_write_width(0, byte_size)
}

pub fn runtime_machine_string_write_width(byte_length: usize) -> usize {
    24 + unsigned_immediate_width(byte_length as u64)
}

pub fn runtime_frame_string_write_width(byte_length: usize) -> usize {
    runtime_machine_string_write_width(byte_length)
}

pub fn runtime_pointee_string_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> usize {
    28 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + unsigned_immediate_width(byte_length as u64)
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

pub fn runtime_storage_address_to_runtime_frame_write_width(
    source_offset: usize,
    target_offset: usize,
) -> usize {
    16 + add_constant_width(source_offset) + store_x_offset_width(target_offset)
}

pub fn runtime_storage_address_to_runtime_frame_target_frame_offset(source_offset: usize) -> usize {
    8 + add_constant_width(source_offset)
}

pub fn runtime_pointee_address_to_runtime_frame_write_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    16 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + store_x_offset_width(target_offset)
}

pub fn runtime_frame_indexed_address_to_runtime_frame_write_width(
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    20 + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + store_x_offset_width(target_offset)
}

pub fn runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    runtime_frame_fixed_index_setup_width(
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
    ) + store_x_offset_width(target_offset)
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_width(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> usize {
    16 + add_constant_width(base_byte_offset)
        // Index is loaded as a 32-bit (4-byte) value, see
        // append_runtime_frame_base_index_target_address.
        + load_data_offset_width(index_offset, 4)
        + scale_index_width(element_byte_size)
        + add_constant_width(field_byte_offset)
        + store_x_offset_width(target_offset)
}

pub fn runtime_text_line_read_import_width(_byte_capacity: usize) -> usize {
    116
}

pub fn runtime_text_line_read_syscall_width(_byte_capacity: usize, syscall_number: u32) -> usize {
    116 + unsigned_immediate_width(u64::from(syscall_number))
}

pub fn runtime_text_line_read_import_target_address_offset() -> usize {
    100
}

pub fn runtime_text_line_read_syscall_target_address_offset(syscall_number: u32) -> usize {
    100 + unsigned_immediate_width(u64::from(syscall_number))
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

pub fn runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(source_field_byte_offset);
    16 + add_constant_width(source_offset)
        + add_constant_width(target_field_byte_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
    element_byte_size: usize,
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    // index setup (x16 = element source-field addr) + load x20 = pointer (4)
    // + add target field to x20 + data copy.
    runtime_frame_index_setup_width(element_byte_size, source_field_byte_offset)
        + 4
        + add_constant_width(target_field_byte_offset)
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
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> usize {
    20 + add_constant_width(pointer_byte_offset)
        + add_constant_width(field_byte_offset)
        + add_constant_width(source_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> usize {
    16 + add_constant_width(pointer_byte_offset)
        + runtime_load_data_width(8)
        + add_constant_width(field_byte_offset)
        + add_constant_width(target_offset)
        + runtime_storage_copy_data_width(0, 0, byte_count)
}

pub fn operand_width(operand: &Aarch64CallOperand) -> usize {
    match operand {
        DataAddress { .. } => 8,
        RuntimeStringPointer { .. } | RuntimeStringLength { .. } | RuntimeScalarInteger { .. } => 12,
        RuntimePointeeStringPointer { .. } | RuntimePointeeStringLength { .. } => 16,
        ImmediateInteger(value) => immediate_width(*value),
        ByteLength(value) => unsigned_immediate_width(*value as u64),
    }
}

fn immediate_width(value: i64) -> usize {
    // Negative values materialize as their full 64-bit two's-complement bit
    // pattern (see `append_unsigned_immediate`), so size that pattern.
    unsigned_immediate_width(value as u64)
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

        width += load_data_offset_width(source_offset, chunk_size)
            + store_data_offset_width(target_offset, chunk_size);
        offset += chunk_size;
        remaining -= chunk_size;
    }

    width
}

fn load_data_offset_width(byte_offset: usize, byte_size: usize) -> usize {
    if data_offset_encodable(byte_offset, byte_size) {
        4
    } else {
        4 + add_constant_width(byte_offset) + 4
    }
}

fn store_data_offset_width(byte_offset: usize, byte_size: usize) -> usize {
    if data_offset_encodable(byte_offset, byte_size) {
        4
    } else {
        4 + add_constant_width(byte_offset) + 4
    }
}

fn data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        2 => byte_offset.is_multiple_of(2) && byte_offset / 2 <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}

fn runtime_text_descriptor_load_pair_width(byte_offset: usize) -> usize {
    load_data_offset_width(byte_offset, 8) + load_data_offset_width(byte_offset + 8, 8)
}

fn runtime_text_descriptor_store_pair_width(byte_offset: usize) -> usize {
    store_data_offset_width(byte_offset, 8) + store_data_offset_width(byte_offset + 8, 8)
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
    } else if let Some((
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        runtime_frame_base_indexed_integer_write_width(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ) - runtime_store_data_width(byte_size)
            + runtime_load_data_width(byte_size)
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        runtime_frame_fixed_index_setup_width(
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        ) + runtime_load_data_width(byte_size)
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let operation_width = if runtime_value_operands.binary_is_float(operand) {
            runtime_float_binary_operation_width()
        } else {
            runtime_binary_operation_width(operator)
        };
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + operation_width
    } else if let Some((
        source,
        source_byte_size,
        target_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
    )) = runtime_value_operands.convert(operand)
    {
        runtime_value_operand_width(runtime_value_operands, source)
            + runtime_convert_operation_width(
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
            )
    } else {
        0
    }
}

fn runtime_binary_operation_width(operator: StateGuardOperator) -> usize {
    // Every operation emits the same instruction count for the 32-bit and
    // 64-bit register forms, so this width is operand-width independent.
    match operator {
        StateGuardOperator::Add
        | StateGuardOperator::And
        | StateGuardOperator::Or
        | StateGuardOperator::Subtract
        | StateGuardOperator::Multiply
        | StateGuardOperator::Divide
        | StateGuardOperator::DivideUnsigned
        | StateGuardOperator::ShiftLeft
        | StateGuardOperator::ShiftRight
        | StateGuardOperator::ShiftRightLogical => 4,
        StateGuardOperator::Modulo | StateGuardOperator::ModuloUnsigned => 8,
        StateGuardOperator::Max
        | StateGuardOperator::Min
        | StateGuardOperator::MaxUnsigned
        | StateGuardOperator::MinUnsigned => 12,
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual
        | StateGuardOperator::GreaterUnsigned
        | StateGuardOperator::GreaterOrEqualUnsigned
        | StateGuardOperator::LessUnsigned
        | StateGuardOperator::LessOrEqualUnsigned => 16,
        _ => 0,
    }
}

fn runtime_store_data_width(byte_size: usize) -> usize {
    // Narrow stores materialize the value as a fixed-width MOVZ+MOVK pair (8)
    // + the sized store (4); 8-byte stores use the padded 4-instruction
    // materialization (16) + STR (4).
    match byte_size {
        1 | 2 | 4 => 12,
        8 => 20,
        _ => 0,
    }
}

fn runtime_load_data_width(byte_size: usize) -> usize {
    match byte_size {
        1 | 2 | 4 | 8 => 4,
        _ => 0,
    }
}

fn runtime_result_write_width(byte_offset: usize, byte_size: usize) -> usize {
    match byte_size {
        1 | 2 | 4 | 8 => store_data_offset_width(byte_offset, byte_size),
        _ => 0,
    }
}

pub(in crate::aarch64) fn runtime_frame_index_setup_width(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    60 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset)
}

fn runtime_frame_fixed_index_setup_width(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    let source_offset = element_index
        .saturating_mul(element_byte_size)
        .saturating_add(field_byte_offset);

    8 + load_data_offset_width(descriptor_offset, 8) + add_constant_width(source_offset)
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

fn store_x_offset_width(byte_offset: usize) -> usize {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        4
    } else {
        add_constant_width(byte_offset) + 4
    }
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
