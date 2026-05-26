use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};

use super::primitives::{
    append_add_x_constant, append_unsigned_immediate, append_unsigned_immediate_padded,
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_add_x_register,
    encode_adrp_placeholder, encode_and_x_register, encode_compare_w_register,
    encode_compare_w17_immediate, encode_compare_x_register, encode_compare_x17_immediate,
    encode_conditional_branch_equal, encode_conditional_branch_greater,
    encode_conditional_branch_greater_or_equal, encode_conditional_branch_less,
    encode_conditional_branch_less_or_equal, encode_conditional_branch_not_equal,
    encode_load_w_from_x, encode_load_x_from_x, encode_move_x_register, encode_movz_w,
    encode_msub_x_register, encode_mul_x_register, encode_orr_x_register, encode_store_w_to_x,
    encode_store_w17_to_x16, encode_store_x_to_x, encode_store_x17_to_x16, encode_sub_x_register,
    encode_udiv_x_register,
};
use super::widths::{
    runtime_frame_base_indexed_address_to_runtime_frame_write_width,
    runtime_frame_base_indexed_binary_write_width, runtime_frame_base_indexed_integer_write_width,
    runtime_frame_indexed_address_to_runtime_frame_write_width,
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_frame_indexed_string_write_width, runtime_machine_indexed_integer_write_width,
    runtime_machine_indexed_string_write_width, runtime_machine_integer_write_width,
    runtime_machine_string_write_width, runtime_pointee_address_to_runtime_frame_write_width,
    runtime_pointee_binary_write_width, runtime_pointee_integer_write_width,
    runtime_pointee_string_write_width, runtime_storage_address_to_runtime_frame_write_width,
    runtime_storage_binary_write_width, runtime_storage_compare_width,
    runtime_storage_copy_to_runtime_pointee_width, runtime_storage_copy_width,
    runtime_storage_value_compare_width, runtime_value_operand_width,
};

pub fn encode_runtime_storage_compare_bytes(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<[u8; 32], Diagnostic> {
    let mut bytes = [0; 32];
    let mut cursor = 0usize;
    append_fixed_instruction(&mut bytes, &mut cursor, encode_adrp_placeholder(16));
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_add_page_offset_placeholder(16),
    );
    append_fixed_instruction(&mut bytes, &mut cursor, encode_adrp_placeholder(17));
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_add_page_offset_placeholder(17),
    );
    match byte_size {
        1 | 4 => {
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_w_from_x(18, 16, left_offset, byte_size)?,
            );
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_w_from_x(19, 17, right_offset, byte_size)?,
            );
            append_fixed_instruction(&mut bytes, &mut cursor, encode_compare_w_register(18, 19));
        }
        8 => {
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_x_from_x(18, 16, left_offset)?,
            );
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_x_from_x(19, 17, right_offset)?,
            );
            append_fixed_instruction(&mut bytes, &mut cursor, encode_compare_x_register(18, 19));
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte runtime guard operands yet"
            )));
        }
    }
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_conditional_branch_for_operator_bytes(operator, failure_branch_distance)?,
    );
    debug_assert_eq!(cursor, runtime_storage_compare_width());
    Ok(bytes)
}

pub fn encode_runtime_storage_value_compare_bytes(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<[u8; 20], Diagnostic> {
    let mut bytes = [0; 20];
    let mut cursor = 0usize;
    append_fixed_instruction(&mut bytes, &mut cursor, encode_adrp_placeholder(16));
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_add_page_offset_placeholder(16),
    );
    match byte_size {
        1 | 4 => {
            let expected_value = u32::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative runtime guard value `{expected_value}` yet"
                ))
            })?;
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_w_from_x(17, 16, byte_offset, byte_size)?,
            );
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_compare_w17_immediate(expected_value)?,
            );
        }
        8 => {
            let expected_value = u64::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative runtime guard value `{expected_value}` yet"
                ))
            })?;
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_load_x_from_x(17, 16, byte_offset)?,
            );
            append_fixed_instruction(
                &mut bytes,
                &mut cursor,
                encode_compare_x17_immediate(expected_value)?,
            );
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte runtime guard values yet"
            )));
        }
    }
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_conditional_branch_for_operator_bytes(operator, failure_branch_distance)?,
    );
    debug_assert_eq!(cursor, runtime_storage_value_compare_width());
    Ok(bytes)
}

pub fn encode_runtime_value_compare(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_value_operand_width(runtime_value_operands, left)
            + runtime_value_operand_width(runtime_value_operands, right)
            + 8,
    );
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 17, &[18, 15, 14], left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 18, &[15, 14], right)?;
    match byte_size {
        1 | 4 => bytes.extend(encode_compare_w_register(17, 18)),
        8 => bytes.extend(encode_compare_x_register(17, 18)),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare computed runtime values of width `{byte_size}` yet"
            )));
        }
    }
    bytes.extend(encode_conditional_branch_for_operator_bytes(
        operator,
        failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_machine_integer_write(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store runtime integer value `{value}` yet"
        ))
    })?;

    let mut bytes = Vec::with_capacity(runtime_machine_integer_write_width(byte_offset, byte_size));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 16, byte_offset)?;
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w17_to_x16(0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value);
            bytes.extend(encode_store_x17_to_x16(0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }
    Ok(bytes)
}

pub fn encode_runtime_pointee_integer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store runtime integer value `{value}` yet"
        ))
    })?;

    let mut bytes = Vec::with_capacity(runtime_pointee_integer_write_width(
        field_byte_offset,
        byte_size,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime pointee integers yet"
            )));
        }
    }
    Ok(bytes)
}

pub fn encode_runtime_storage_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        operator,
        right,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 17, &[18, 15, 14], left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 18, &[15, 14], right)?;
    append_runtime_binary_operation(&mut bytes, 17, operator, 18)?;
    append_runtime_storage_result_write(&mut bytes, target_offset, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_pointee_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_binary_write_width(
        runtime_value_operands,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 17, &[18, 15, 14], left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 18, &[15, 14], right)?;
    append_runtime_binary_operation(&mut bytes, 17, operator, 18)?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_string_write_width(byte_length));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x17_to_x16(byte_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_pointee_string_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_string_write_width(
        field_byte_offset,
        byte_length,
    ));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_string_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_string_write_width(
        element_byte_size,
        field_byte_offset,
        byte_length,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_machine_indexed_string_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_string_write_width(
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        byte_length,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_storage_address_to_runtime_frame_write(
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_address_to_runtime_frame_write_width());
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_add_x_immediate(17, 17, source_offset)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x_to_x(17, 16, target_offset)?);
    Ok(bytes)
}

pub fn encode_runtime_pointee_address_to_runtime_frame_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_address_to_runtime_frame_write_width());
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(17, 16, pointer_byte_offset)?);
    bytes.extend(encode_add_x_immediate(17, 17, field_byte_offset)?);
    bytes.extend(encode_store_x_to_x(17, 16, target_offset)?);
    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_address_to_runtime_frame_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_address_to_runtime_frame_write_width(
        element_byte_size,
        field_byte_offset,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_store_x_to_x(16, 20, target_offset)?);
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_frame_base_indexed_address_to_runtime_frame_write_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
        ),
    );
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_store_x_to_x(16, 20, target_offset)?);
    Ok(bytes)
}

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_width(
        source_offset,
        target_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    append_add_constant_to_x_register(&mut bytes, 17, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(18, 16, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(18, 17, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(18, 16, offset)?);
                bytes.extend(encode_store_x_to_x(18, 17, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_integer_write(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store runtime integer value `{value}` yet"
        ))
    })?;

    let mut bytes = Vec::with_capacity(runtime_frame_indexed_integer_write_width(
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }

    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store runtime indexed integer value `{value}` yet"
        ))
    })?;

    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_integer_write_width(
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w17_to_x16(0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value);
            bytes.extend(encode_store_x17_to_x16(0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime indexed integers yet"
            )));
        }
    }
    Ok(bytes)
}

pub fn encode_runtime_machine_indexed_integer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store runtime integer value `{value}` yet"
        ))
    })?;

    let mut bytes = Vec::with_capacity(runtime_machine_indexed_integer_write_width(
        base_byte_offset,
        index_region,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }

    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_binary_write_width(
        runtime_value_operands,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 17, &[18, 15, 14], left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 18, &[15, 14], right)?;
    append_runtime_binary_operation(&mut bytes, 17, operator, 18)?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_base_indexed_binary_write_width(
        runtime_value_operands,
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    ));
    append_runtime_frame_base_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 17, &[18, 15, 14], left)?;
    append_runtime_value_operand(runtime_value_operands, &mut bytes, 18, &[15, 14], right)?;
    append_runtime_binary_operation(&mut bytes, 17, operator, 18)?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_to_runtime_frame_indexed_width(
            source_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 20, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 20, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 16, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 20, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, offset)?);
                bytes.extend(encode_store_x_to_x(17, 20, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 16, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 20, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, offset)?);
                bytes.extend(encode_store_x_to_x(17, 20, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 16, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 20, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, offset)?);
                bytes.extend(encode_store_x_to_x(17, 20, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 16, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 20, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, offset)?);
                bytes.extend(encode_store_x_to_x(17, 20, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        super::widths::runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 16, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 20, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, offset)?);
                bytes.extend(encode_store_x_to_x(17, 20, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_runtime_pointee_width(
        source_offset,
        field_byte_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
    }

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 20, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 20, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
        Ok(())
    })?;

    Ok(bytes)
}

fn for_each_runtime_copy_chunk(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
    mut visit: impl FnMut(usize, usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;

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

        visit(offset, chunk_size)?;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    if offset != byte_count {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) of runtime storage yet"
        )));
    }

    Ok(())
}

fn append_runtime_frame_index_target_address(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    bytes.extend(encode_load_x_from_x(17, 20, index_offset)?);
    append_scale_x_register_by_constant(bytes, 18, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 18));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_runtime_frame_fixed_index_target_address(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    let scaled_index = element_index
        .checked_mul(element_byte_size)
        .ok_or_else(|| {
            Diagnostic::error(
                "AArch64 MVP encoder cannot address overflowing fixed indexed operand",
            )
        })?;
    let byte_offset = scaled_index.checked_add(field_byte_offset).ok_or_else(|| {
        Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed operand")
    })?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_runtime_storage_load(bytes, 16, 20, descriptor_offset, 8, "runtime frame indexed")?;
    append_add_constant_to_x_register(bytes, 16, byte_offset)?;
    Ok(())
}

fn append_runtime_machine_index_target_address(
    bytes: &mut Vec<u8>,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    append_add_constant_to_x_register(bytes, 16, base_byte_offset)?;
    match index_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
            bytes.extend(encode_adrp_placeholder(20));
            bytes.extend(encode_add_page_offset_placeholder(20));
            bytes.extend(encode_load_x_from_x(17, 20, index_offset)?);
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            bytes.extend(encode_load_x_from_x(17, 20, index_offset)?);
        }
    }
    append_scale_x_register_by_constant(bytes, 18, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 18));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_runtime_frame_base_index_target_address(
    bytes: &mut Vec<u8>,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(16, 20));
    append_add_constant_to_x_register(bytes, 16, base_byte_offset)?;
    bytes.extend(encode_load_x_from_x(17, 20, index_offset)?);
    append_scale_x_register_by_constant(bytes, 18, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 18));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_runtime_value_operand(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    bytes: &mut Vec<u8>,
    destination_register: u8,
    scratch_registers: &[u8],
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    if let Some(value) = runtime_value_operands.immediate_integer(operand) {
        let value = u64::try_from(value).map_err(|_| {
            Diagnostic::error(format!(
                "AArch64 MVP encoder cannot materialize runtime immediate `{value}` yet"
            ))
        })?;
        append_unsigned_immediate(bytes, destination_register, value);
        Ok(())
    } else if let Some((_, byte_offset, byte_size)) = runtime_value_operands.storage(operand) {
        bytes.extend(encode_adrp_placeholder(19));
        bytes.extend(encode_add_page_offset_placeholder(19));
        append_runtime_storage_load(
            bytes,
            destination_register,
            19,
            byte_offset,
            byte_size,
            "runtime operand",
        )?;
        Ok(())
    } else if let Some((pointer_byte_offset, field_byte_offset, byte_size)) =
        runtime_value_operands.pointee(operand)
    {
        bytes.extend(encode_adrp_placeholder(19));
        bytes.extend(encode_add_page_offset_placeholder(19));
        append_runtime_storage_load(bytes, 19, 19, pointer_byte_offset, 8, "runtime pointee")?;
        if field_byte_offset > 0 {
            append_add_constant_to_x_register(bytes, 19, field_byte_offset)?;
        }
        append_runtime_storage_load(
            bytes,
            destination_register,
            19,
            0,
            byte_size,
            "runtime pointee operand",
        )?;
        Ok(())
    } else if let Some((
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_indexed(operand)
    {
        append_runtime_frame_index_target_address(
            bytes,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                16,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((
        base_byte_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_base_indexed(operand)
    {
        append_runtime_frame_base_index_target_address(
            bytes,
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                16,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime frame-base-indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
        byte_size,
    )) = runtime_value_operands.frame_fixed_indexed(operand)
    {
        append_runtime_frame_fixed_index_target_address(
            bytes,
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
        )?;
        match byte_size {
            1 | 4 => bytes.extend(encode_load_w_from_x(
                destination_register,
                16,
                0,
                byte_size,
            )?),
            8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
            _ => {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load runtime fixed indexed operand width `{byte_size}` yet"
                )));
            }
        }
        Ok(())
    } else if let Some((left, operator, right)) = runtime_value_operands.binary(operand) {
        let Some((&rhs_register, remaining_scratch)) = scratch_registers.split_first() else {
            return Err(Diagnostic::error(
                "AArch64 MVP encoder ran out of scratch registers for runtime arithmetic",
            ));
        };

        append_runtime_value_operand(
            runtime_value_operands,
            bytes,
            destination_register,
            scratch_registers,
            left,
        )?;
        append_runtime_value_operand(
            runtime_value_operands,
            bytes,
            rhs_register,
            remaining_scratch,
            right,
        )?;
        append_runtime_binary_operation(bytes, destination_register, operator, rhs_register)?;
        Ok(())
    } else {
        Err(Diagnostic::error(
            "AArch64 runtime value operand is not implemented yet",
        ))
    }
}

fn append_runtime_storage_load(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
    context: &str,
) -> Result<(), Diagnostic> {
    if byte_offset > 0 {
        append_add_constant_to_x_register(bytes, base_register, byte_offset)?;
    }

    match byte_size {
        1 | 4 => bytes.extend(encode_load_w_from_x(
            destination_register,
            base_register,
            0,
            byte_size,
        )?),
        8 => bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            0,
        )?),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot load {context} width `{byte_size}` yet"
            )));
        }
    }

    Ok(())
}

fn append_runtime_binary_operation(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    operator: StateGuardOperator,
    right_register: u8,
) -> Result<(), Diagnostic> {
    match operator {
        StateGuardOperator::Add => {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::And => {
            bytes.extend(encode_and_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Or => {
            bytes.extend(encode_orr_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Subtract => {
            bytes.extend(encode_sub_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Multiply => {
            bytes.extend(encode_mul_x_register(
                destination_register,
                destination_register,
                right_register,
            ));
        }
        StateGuardOperator::Modulo => {
            bytes.extend(encode_udiv_x_register(
                19,
                destination_register,
                right_register,
            ));
            bytes.extend(encode_msub_x_register(
                destination_register,
                19,
                right_register,
                destination_register,
            ));
        }
        StateGuardOperator::Max | StateGuardOperator::Min => {
            bytes.extend(encode_compare_x_register(
                destination_register,
                right_register,
            ));
            bytes.extend(match operator {
                StateGuardOperator::Max => encode_conditional_branch_greater_or_equal(8)?,
                StateGuardOperator::Min => encode_conditional_branch_less_or_equal(8)?,
                _ => unreachable!(),
            });
            bytes.extend(encode_move_x_register(destination_register, right_register));
        }
        StateGuardOperator::Equal
        | StateGuardOperator::NotEqual
        | StateGuardOperator::Greater
        | StateGuardOperator::GreaterOrEqual
        | StateGuardOperator::Less
        | StateGuardOperator::LessOrEqual => {
            bytes.extend(encode_compare_w_register(
                destination_register,
                right_register,
            ));
            bytes.extend(encode_movz_w(destination_register, 0));
            bytes.extend(match operator {
                StateGuardOperator::Equal => encode_conditional_branch_not_equal(8)?,
                StateGuardOperator::NotEqual => encode_conditional_branch_equal(8)?,
                StateGuardOperator::Greater => encode_conditional_branch_less_or_equal(8)?,
                StateGuardOperator::GreaterOrEqual => encode_conditional_branch_less(8)?,
                StateGuardOperator::Less => encode_conditional_branch_greater_or_equal(8)?,
                StateGuardOperator::LessOrEqual => encode_conditional_branch_greater(8)?,
                _ => unreachable!(),
            });
            bytes.extend(encode_movz_w(destination_register, 1));
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot lower runtime binary operator `{operator:?}` yet"
            )));
        }
    }

    Ok(())
}

fn append_runtime_storage_result_write(
    bytes: &mut Vec<u8>,
    byte_offset: usize,
    byte_size: usize,
) -> Result<(), Diagnostic> {
    match byte_size {
        1 | 4 => bytes.extend(encode_store_w_to_x(17, 16, byte_offset, byte_size)?),
        8 => bytes.extend(encode_store_x_to_x(17, 16, byte_offset)?),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot write {byte_size}-byte runtime storage results yet"
            )));
        }
    }

    Ok(())
}

fn encode_conditional_branch_for_operator_bytes(
    operator: StateGuardOperator,
    failure_branch_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    Ok(match operator {
        StateGuardOperator::Equal => encode_conditional_branch_not_equal(failure_branch_distance)?,
        StateGuardOperator::NotEqual => encode_conditional_branch_equal(failure_branch_distance)?,
        StateGuardOperator::Greater => {
            encode_conditional_branch_less_or_equal(failure_branch_distance)?
        }
        StateGuardOperator::GreaterOrEqual => {
            encode_conditional_branch_less(failure_branch_distance)?
        }
        StateGuardOperator::Less => {
            encode_conditional_branch_greater_or_equal(failure_branch_distance)?
        }
        StateGuardOperator::LessOrEqual => {
            encode_conditional_branch_greater(failure_branch_distance)?
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot lower runtime compare operator `{operator:?}` yet"
        )))?,
    })
}

fn append_fixed_instruction<const BYTE_COUNT: usize>(
    bytes: &mut [u8; BYTE_COUNT],
    cursor: &mut usize,
    instruction: [u8; 4],
) {
    bytes[*cursor..*cursor + 4].copy_from_slice(&instruction);
    *cursor += 4;
}

fn append_scale_x_register_by_constant(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    source_register: u8,
    scale: usize,
) -> Result<(), Diagnostic> {
    if scale == 0 {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot scale indexed runtime storage by zero",
        ));
    }

    append_unsigned_immediate(bytes, destination_register, 0);
    let working_register = 19u8;
    bytes.extend(encode_move_x_register(working_register, source_register));

    let highest_bit = usize::BITS - scale.leading_zeros();
    for bit_index in 0..highest_bit {
        if (scale >> bit_index) & 1 == 1 {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                working_register,
            ));
        }

        if bit_index + 1 < highest_bit {
            bytes.extend(encode_add_x_register(
                working_register,
                working_register,
                working_register,
            ));
        }
    }

    Ok(())
}

fn append_add_constant_to_x_register(
    bytes: &mut Vec<u8>,
    register: u8,
    value: usize,
) -> Result<(), Diagnostic> {
    let scratch_register = if register == 19 { 18 } else { 19 };
    append_add_x_constant(bytes, register, register, value, scratch_register)
}
