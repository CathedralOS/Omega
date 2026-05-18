use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{RuntimeValueOperand, RuntimeValueOperandHandle, StateGuardOperator};

use super::primitives::{
    append_unsigned_immediate, append_unsigned_immediate_padded,
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_add_x_register,
    encode_adrp_placeholder, encode_compare_w_register, encode_compare_w17_immediate,
    encode_compare_x_register, encode_compare_x17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_not_equal, encode_load_w_from_x, encode_load_x_from_x,
    encode_move_x_register, encode_movz_w, encode_msub_x_register, encode_mul_x_register,
    encode_store_w_to_x, encode_store_w17_to_x16, encode_store_x_to_x, encode_store_x17_to_x16,
    encode_sub_x_register, encode_udiv_x_register,
};
use super::widths::{
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_machine_integer_write_width, runtime_machine_string_write_width,
    runtime_pointee_binary_write_width, runtime_pointee_integer_write_width,
    runtime_pointee_string_write_width, runtime_storage_binary_write_width,
    runtime_storage_compare_width, runtime_storage_copy_to_runtime_pointee_width,
    runtime_storage_copy_width, runtime_storage_value_compare_width, runtime_value_operand_width,
};

pub fn encode_runtime_storage_compare(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_compare_width());
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_load_w_from_x(18, 16, left_offset, byte_size)?);
            bytes.extend(encode_load_w_from_x(19, 17, right_offset, byte_size)?);
            bytes.extend(encode_compare_w_register(18, 19));
        }
        8 => {
            bytes.extend(encode_load_x_from_x(18, 16, left_offset)?);
            bytes.extend(encode_load_x_from_x(19, 17, right_offset)?);
            bytes.extend(encode_compare_x_register(18, 19));
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte runtime guard operands yet"
            )));
        }
    }
    bytes.extend(encode_conditional_branch_for_operator_bytes(
        operator,
        failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_storage_value_compare(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_value_compare_width());
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_size {
        1 | 4 => {
            let expected_value = u32::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative runtime guard value `{expected_value}` yet"
                ))
            })?;
            bytes.extend(encode_load_w_from_x(17, 16, byte_offset, byte_size)?);
            bytes.extend(encode_compare_w17_immediate(expected_value)?);
        }
        8 => {
            let expected_value = u64::try_from(expected_value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot compare negative runtime guard value `{expected_value}` yet"
                ))
            })?;
            bytes.extend(encode_load_x_from_x(17, 16, byte_offset)?);
            bytes.extend(encode_compare_x17_immediate(expected_value)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare {byte_size}-byte runtime guard values yet"
            )));
        }
    }
    bytes.extend(encode_conditional_branch_for_operator_bytes(
        operator,
        failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_value_compare(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
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

    let mut bytes = Vec::with_capacity(runtime_machine_integer_write_width(byte_size));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w17_to_x16(byte_offset, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value);
            bytes.extend(encode_store_x17_to_x16(byte_offset)?);
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

    let mut bytes = Vec::with_capacity(runtime_pointee_integer_write_width(byte_size));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
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
    runtime_value_operands: &Arena<RuntimeValueOperand>,
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
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_binary_write_width(
        runtime_value_operands,
        byte_size,
        left,
        right,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
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
    let mut bytes = Vec::with_capacity(runtime_pointee_string_write_width(byte_length));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
    }
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_width(byte_count));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));

    for_each_runtime_copy_chunk(
        source_offset,
        target_offset,
        byte_count,
        |offset, chunk_size| {
            match chunk_size {
                1 | 4 => {
                    bytes.extend(encode_load_w_from_x(
                        18,
                        16,
                        source_offset + offset,
                        chunk_size,
                    )?);
                    bytes.extend(encode_store_w_to_x(
                        18,
                        17,
                        target_offset + offset,
                        chunk_size,
                    )?);
                }
                8 => {
                    bytes.extend(encode_load_x_from_x(18, 16, source_offset + offset)?);
                    bytes.extend(encode_store_x_to_x(18, 17, target_offset + offset)?);
                }
                _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
            }
            Ok(())
        },
    )?;

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

pub fn encode_runtime_frame_indexed_binary_write(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
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

    for_each_runtime_copy_chunk(
        source_offset,
        field_byte_offset,
        byte_count,
        |offset, chunk_size| {
            match chunk_size {
                1 | 4 => {
                    bytes.extend(encode_load_w_from_x(
                        17,
                        20,
                        source_offset + offset,
                        chunk_size,
                    )?);
                    bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
                }
                8 => {
                    bytes.extend(encode_load_x_from_x(17, 20, source_offset + offset)?);
                    bytes.extend(encode_store_x_to_x(17, 16, offset)?);
                }
                _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
            }
            Ok(())
        },
    )?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_runtime_pointee_width(
        field_byte_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
    }

    for_each_runtime_copy_chunk(
        source_offset,
        field_byte_offset,
        byte_count,
        |offset, chunk_size| {
            match chunk_size {
                1 | 4 => {
                    bytes.extend(encode_load_w_from_x(
                        17,
                        20,
                        source_offset + offset,
                        chunk_size,
                    )?);
                    bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
                }
                8 => {
                    bytes.extend(encode_load_x_from_x(17, 20, source_offset + offset)?);
                    bytes.extend(encode_store_x_to_x(17, 16, offset)?);
                }
                _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
            }
            Ok(())
        },
    )?;

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

fn append_runtime_value_operand(
    runtime_value_operands: &Arena<RuntimeValueOperand>,
    bytes: &mut Vec<u8>,
    destination_register: u8,
    scratch_registers: &[u8],
    operand: RuntimeValueOperandHandle,
) -> Result<(), Diagnostic> {
    match runtime_value_operands.get(operand) {
        RuntimeValueOperand::Immediate(value) => {
            let value = u64::try_from(*value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot materialize runtime immediate `{value}` yet"
                ))
            })?;
            append_unsigned_immediate(bytes, destination_register, value);
            Ok(())
        }
        RuntimeValueOperand::Storage {
            byte_offset,
            byte_size,
            ..
        } => {
            bytes.extend(encode_adrp_placeholder(19));
            bytes.extend(encode_add_page_offset_placeholder(19));
            match byte_size {
                1 | 4 => bytes.extend(encode_load_w_from_x(
                    destination_register,
                    19,
                    *byte_offset,
                    *byte_size,
                )?),
                8 => bytes.extend(encode_load_x_from_x(
                    destination_register,
                    19,
                    *byte_offset,
                )?),
                _ => {
                    return Err(Diagnostic::error(format!(
                        "AArch64 MVP encoder cannot load runtime operand width `{byte_size}` yet"
                    )));
                }
            }
            Ok(())
        }
        RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            bytes.extend(encode_adrp_placeholder(19));
            bytes.extend(encode_add_page_offset_placeholder(19));
            bytes.extend(encode_load_x_from_x(19, 19, *pointer_byte_offset)?);
            if *field_byte_offset > 0 {
                bytes.extend(encode_add_x_immediate(19, 19, *field_byte_offset)?);
            }
            match byte_size {
                1 | 4 => bytes.extend(encode_load_w_from_x(
                    destination_register,
                    19,
                    0,
                    *byte_size,
                )?),
                8 => bytes.extend(encode_load_x_from_x(destination_register, 19, 0)?),
                _ => {
                    return Err(Diagnostic::error(format!(
                        "AArch64 MVP encoder cannot load runtime pointee operand width `{byte_size}` yet"
                    )));
                }
            }
            Ok(())
        }
        RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            append_runtime_frame_index_target_address(
                bytes,
                *descriptor_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
            )?;
            match byte_size {
                1 | 4 => bytes.extend(encode_load_w_from_x(
                    destination_register,
                    16,
                    0,
                    *byte_size,
                )?),
                8 => bytes.extend(encode_load_x_from_x(destination_register, 16, 0)?),
                _ => {
                    return Err(Diagnostic::error(format!(
                        "AArch64 MVP encoder cannot load runtime indexed operand width `{byte_size}` yet"
                    )));
                }
            }
            Ok(())
        }
        RuntimeValueOperand::Binary {
            left,
            operator,
            right,
        } => {
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
                *left,
            )?;
            append_runtime_value_operand(
                runtime_value_operands,
                bytes,
                rhs_register,
                remaining_scratch,
                *right,
            )?;
            append_runtime_binary_operation(bytes, destination_register, *operator, rhs_register)?;
            Ok(())
        }
    }
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
        StateGuardOperator::Equal => encode_conditional_branch_equal(failure_branch_distance)?,
        StateGuardOperator::NotEqual => {
            encode_conditional_branch_not_equal(failure_branch_distance)?
        }
        StateGuardOperator::Greater => encode_conditional_branch_greater(failure_branch_distance)?,
        StateGuardOperator::GreaterOrEqual => {
            encode_conditional_branch_greater_or_equal(failure_branch_distance)?
        }
        StateGuardOperator::Less => encode_conditional_branch_less(failure_branch_distance)?,
        StateGuardOperator::LessOrEqual => {
            encode_conditional_branch_less_or_equal(failure_branch_distance)?
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot lower runtime compare operator `{operator:?}` yet"
        )))?,
    })
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
    if value == 0 {
        return Ok(());
    }
    if value <= 4095 {
        bytes.extend(encode_add_x_immediate(register, register, value)?);
        return Ok(());
    }

    append_unsigned_immediate(bytes, 19, value as u64);
    bytes.extend(encode_add_x_register(register, register, 19));
    Ok(())
}
