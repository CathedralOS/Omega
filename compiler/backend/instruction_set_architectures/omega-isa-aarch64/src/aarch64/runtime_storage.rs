use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{RuntimeValueOperand, StateGuardOperator};

use super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w_register,
    encode_compare_w17_immediate, encode_compare_x_register, encode_compare_x17_immediate,
    encode_conditional_branch_equal,
    encode_conditional_branch_greater, encode_conditional_branch_greater_or_equal,
    encode_conditional_branch_less, encode_conditional_branch_less_or_equal,
    encode_conditional_branch_not_equal, encode_load_w_from_x, encode_load_x_from_x,
    encode_move_x_register, encode_movz_w, encode_msub_x_register, encode_mul_x_register,
    encode_store_w_to_x, encode_store_w17_to_x16,
    encode_store_x_to_x, encode_store_x17_to_x16, encode_unsigned_immediate,
    encode_unsigned_immediate_padded, encode_add_x_immediate, encode_add_x_register,
    encode_sub_x_register, encode_udiv_x_register,
};

pub fn encode_runtime_storage_compare(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
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
    bytes.extend(encode_conditional_branch_for_operator(
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
    let mut bytes = encode_adrp_placeholder(16);
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
    bytes.extend(encode_conditional_branch_for_operator(
        operator,
        failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_value_compare(
    left: &RuntimeValueOperand,
    right: &RuntimeValueOperand,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_runtime_value_operand(17, &[18, 15, 14], left)?;
    bytes.extend(encode_runtime_value_operand(18, &[15, 14], right)?);
    match byte_size {
        1 | 4 => bytes.extend(encode_compare_w_register(17, 18)),
        8 => bytes.extend(encode_compare_x_register(17, 18)),
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot compare computed runtime values of width `{byte_size}` yet"
            )));
        }
    }
    bytes.extend(encode_conditional_branch_for_operator(
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

    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_size {
        1 | 4 => {
            bytes.extend(encode_movz_w(17, value as u16));
            bytes.extend(encode_store_w17_to_x16(byte_offset, byte_size)?);
        }
        8 => {
            bytes.extend(encode_unsigned_immediate_padded(17, value));
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

    let mut bytes = encode_adrp_placeholder(16);
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
            bytes.extend(encode_unsigned_immediate_padded(17, value));
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
    target_offset: usize,
    byte_size: usize,
    left: &RuntimeValueOperand,
    operator: StateGuardOperator,
    right: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_runtime_value_operand(17, &[18, 15, 14], left)?);
    bytes.extend(encode_runtime_value_operand(18, &[15, 14], right)?);
    bytes.extend(encode_runtime_binary_operation(17, operator, 18)?);
    bytes.extend(encode_runtime_storage_result_write(target_offset, byte_size));
    Ok(bytes)
}

pub fn encode_runtime_pointee_binary_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: &RuntimeValueOperand,
    operator: StateGuardOperator,
    right: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
    }
    bytes.extend(encode_runtime_value_operand(17, &[18, 15, 14], left)?);
    bytes.extend(encode_runtime_value_operand(18, &[15, 14], right)?);
    bytes.extend(encode_runtime_binary_operation(17, operator, 18)?);
    bytes.extend(encode_runtime_storage_result_write(0, byte_size));
    Ok(bytes)
}

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(17);
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    bytes.extend(encode_unsigned_immediate(17, byte_length as u64));
    bytes.extend(encode_store_x17_to_x16(byte_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_pointee_string_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(17);
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(16, 16, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(16, 16, field_byte_offset)?);
    }
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    bytes.extend(encode_unsigned_immediate(17, byte_length as u64));
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));

    for (offset, chunk_size) in runtime_copy_chunks(source_offset, target_offset, byte_count)? {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(18, 16, source_offset + offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(18, 17, target_offset + offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(18, 16, source_offset + offset)?);
                bytes.extend(encode_store_x_to_x(18, 17, target_offset + offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
    }

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

    let mut bytes = encode_runtime_frame_index_target_address(
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
            bytes.extend(encode_unsigned_immediate_padded(17, value));
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
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: &RuntimeValueOperand,
    operator: StateGuardOperator,
    right: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_runtime_frame_index_target_address(
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_runtime_value_operand(17, &[18, 15, 14], left)?);
    bytes.extend(encode_runtime_value_operand(18, &[15, 14], right)?);
    bytes.extend(encode_runtime_binary_operation(17, operator, 18)?);
    bytes.extend(encode_runtime_storage_result_write(0, byte_size));
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
    let mut bytes = encode_runtime_frame_index_target_address(
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;

    for (offset, chunk_size) in runtime_copy_chunks(source_offset, field_byte_offset, byte_count)? {
        match chunk_size {
            1 | 4 => {
                bytes.extend(encode_load_w_from_x(17, 20, source_offset + offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
            8 => {
                bytes.extend(encode_load_x_from_x(17, 20, source_offset + offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => unreachable!("runtime_copy_chunks only yields 1, 4, or 8 byte chunks"),
        }
    }

    Ok(bytes)
}

fn runtime_copy_chunks(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
) -> Result<Vec<(usize, usize)>, Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;
    let mut chunks = Vec::new();

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size = if remaining >= 8
            && source_offset.is_multiple_of(8)
            && target_offset.is_multiple_of(8)
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

        chunks.push((offset, chunk_size));
        offset += chunk_size;
        remaining -= chunk_size;
    }

    if offset != byte_count {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) of runtime storage yet"
        )));
    }

    Ok(chunks)
}

fn encode_runtime_frame_index_target_address(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(20);
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    bytes.extend(encode_load_x_from_x(17, 20, index_offset)?);
    bytes.extend(encode_scale_x_register_by_constant(18, 17, element_byte_size)?);
    bytes.extend(encode_add_x_register(16, 16, 18));
    bytes.extend(encode_add_constant_to_x_register(16, field_byte_offset)?);
    Ok(bytes)
}

fn encode_runtime_value_operand(
    destination_register: u8,
    scratch_registers: &[u8],
    operand: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    match operand {
        RuntimeValueOperand::Immediate(value) => {
            let value = u64::try_from(*value).map_err(|_| {
                Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot materialize runtime immediate `{value}` yet"
                ))
            })?;
            Ok(encode_unsigned_immediate(destination_register, value))
        }
        RuntimeValueOperand::Storage {
            byte_offset,
            byte_size,
            ..
        } => {
            let mut bytes = encode_adrp_placeholder(19);
            bytes.extend(encode_add_page_offset_placeholder(19));
            match byte_size {
                1 | 4 => bytes.extend(encode_load_w_from_x(
                    destination_register,
                    19,
                    *byte_offset,
                    *byte_size,
                )?),
                8 => bytes.extend(encode_load_x_from_x(destination_register, 19, *byte_offset)?),
                _ => {
                    return Err(Diagnostic::error(format!(
                        "AArch64 MVP encoder cannot load runtime operand width `{byte_size}` yet"
                    )));
                }
            }
            Ok(bytes)
        }
        RuntimeValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => {
            let mut bytes = encode_adrp_placeholder(19);
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
            Ok(bytes)
        }
        RuntimeValueOperand::FrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => {
            let mut bytes = encode_runtime_frame_index_target_address(
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
            Ok(bytes)
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

            let mut bytes =
                encode_runtime_value_operand(destination_register, scratch_registers, left)?;
            bytes.extend(encode_runtime_value_operand(
                rhs_register,
                remaining_scratch,
                right,
            )?);
            bytes.extend(encode_runtime_binary_operation(
                destination_register,
                *operator,
                rhs_register,
            )?);
            Ok(bytes)
        }
    }
}

fn encode_runtime_binary_operation(
    destination_register: u8,
    operator: StateGuardOperator,
    right_register: u8,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();

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
            bytes.extend(encode_udiv_x_register(19, destination_register, right_register));
            bytes.extend(encode_msub_x_register(
                destination_register,
                19,
                right_register,
                destination_register,
            ));
        }
        StateGuardOperator::Max | StateGuardOperator::Min => {
            bytes.extend(encode_compare_x_register(destination_register, right_register));
            bytes.extend(match operator {
                StateGuardOperator::Max => {
                    encode_conditional_branch_greater_or_equal(8)?
                }
                StateGuardOperator::Min => encode_conditional_branch_less_or_equal(8)?,
                _ => unreachable!(),
            });
            bytes.extend(encode_move_x_register(destination_register, right_register));
        }
        StateGuardOperator::Equal | StateGuardOperator::NotEqual => {
            bytes.extend(encode_compare_w_register(destination_register, right_register));
            bytes.extend(encode_movz_w(destination_register, 0));
            bytes.extend(match operator {
                StateGuardOperator::Equal => encode_conditional_branch_not_equal(8)?,
                StateGuardOperator::NotEqual => encode_conditional_branch_equal(8)?,
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

    Ok(bytes)
}

fn encode_runtime_storage_result_write(byte_offset: usize, byte_size: usize) -> Vec<u8> {
    match byte_size {
        1 | 4 => encode_store_w_to_x(17, 16, byte_offset, byte_size)
            .expect("runtime binary write should target a supported integer width"),
        8 => encode_store_x_to_x(17, 16, byte_offset)
            .expect("runtime binary write should target an aligned 8-byte slot"),
        _ => Vec::new(),
    }
}

fn encode_conditional_branch_for_operator(
    operator: StateGuardOperator,
    failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match operator {
        StateGuardOperator::Equal => encode_conditional_branch_equal(failure_branch_distance),
        StateGuardOperator::NotEqual => {
            encode_conditional_branch_not_equal(failure_branch_distance)
        }
        StateGuardOperator::Greater => encode_conditional_branch_greater(failure_branch_distance),
        StateGuardOperator::GreaterOrEqual => {
            encode_conditional_branch_greater_or_equal(failure_branch_distance)
        }
        StateGuardOperator::Less => encode_conditional_branch_less(failure_branch_distance),
        StateGuardOperator::LessOrEqual => {
            encode_conditional_branch_less_or_equal(failure_branch_distance)
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot lower runtime compare operator `{operator:?}` yet"
        ))),
    }
}

fn encode_scale_x_register_by_constant(
    destination_register: u8,
    source_register: u8,
    scale: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if scale == 0 {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot scale indexed runtime storage by zero",
        ));
    }

    let mut bytes = encode_unsigned_immediate(destination_register, 0);
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

    Ok(bytes)
}

fn encode_add_constant_to_x_register(
    register: u8,
    value: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if value == 0 {
        return Ok(Vec::new());
    }
    if value <= 4095 {
        return encode_add_x_immediate(register, register, value);
    }

    let mut bytes = encode_unsigned_immediate(19, value as u64);
    bytes.extend(encode_add_x_register(register, register, 19));
    Ok(bytes)
}
