use omega_core::diagnostics::Diagnostic;
use omega_target_operations::{RuntimeValueOperand, StateGuardOperator};

use super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w_register,
    encode_compare_w17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_w_from_x, encode_load_x_from_x,
    encode_move_x_register, encode_movz_w, encode_store_w_to_x, encode_store_w17_to_x16,
    encode_store_x_to_x, encode_store_x17_to_x16, encode_unsigned_immediate,
    encode_unsigned_immediate_padded, encode_add_x_immediate, encode_add_x_register,
    encode_sub_x_register,
};

pub fn encode_runtime_storage_compare(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_w_from_x(18, 16, left_offset, byte_size)?);
    bytes.extend(encode_load_w_from_x(19, 17, right_offset, byte_size)?);
    bytes.extend(encode_compare_w_register(18, 19));
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(failure_branch_distance)?
    } else {
        encode_conditional_branch_not_equal(failure_branch_distance)?
    });
    Ok(bytes)
}

pub fn encode_runtime_storage_value_compare(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_value = u32::try_from(expected_value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare negative runtime guard value `{expected_value}` yet"
        ))
    })?;

    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_w_from_x(17, 16, byte_offset, byte_size)?);
    bytes.extend(encode_compare_w17_immediate(expected_value)?);
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(failure_branch_distance)?
    } else {
        encode_conditional_branch_not_equal(failure_branch_distance)?
    });
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

pub fn encode_runtime_storage_binary_write(
    target_offset: usize,
    byte_size: usize,
    left: &RuntimeValueOperand,
    operator: StateGuardOperator,
    right: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_runtime_value_operand(17, left)?);
    bytes.extend(encode_runtime_value_operand(18, right)?);
    bytes.extend(encode_runtime_binary_operation(17, operator)?);
    bytes.extend(encode_runtime_storage_result_write(target_offset, byte_size));
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

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));

    match byte_count {
        1 | 4 => {
            bytes.extend(encode_load_w_from_x(18, 16, source_offset, byte_count)?);
            bytes.extend(encode_store_w_to_x(18, 17, target_offset, byte_count)?);
        }
        _ if byte_count.is_multiple_of(8) => {
            for offset in (0..byte_count).step_by(8) {
                bytes.extend(encode_load_x_from_x(18, 16, source_offset + offset)?);
                bytes.extend(encode_store_x_to_x(18, 17, target_offset + offset)?);
            }
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) of runtime storage yet"
            )));
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
    bytes.extend(encode_runtime_value_operand(17, left)?);
    bytes.extend(encode_runtime_value_operand(18, right)?);
    bytes.extend(encode_runtime_binary_operation(17, operator)?);
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

    match byte_count {
        1 | 4 => {
            bytes.extend(encode_load_w_from_x(17, 20, source_offset, byte_count)?);
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_count)?);
        }
        _ if byte_count.is_multiple_of(8) => {
            for offset in (0..byte_count).step_by(8) {
                bytes.extend(encode_load_x_from_x(17, 20, source_offset + offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) into indexed runtime storage yet"
            )));
        }
    }

    Ok(bytes)
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
        RuntimeValueOperand::Storage { byte_offset, .. } => {
            let mut bytes = encode_adrp_placeholder(19);
            bytes.extend(encode_add_page_offset_placeholder(19));
            let byte_size = match operand {
                RuntimeValueOperand::Storage { byte_size, .. } => *byte_size,
                RuntimeValueOperand::Immediate(_) => unreachable!(),
            };
            match byte_size {
                1 | 4 => bytes.extend(encode_load_w_from_x(
                    destination_register,
                    19,
                    *byte_offset,
                    byte_size,
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
    }
}

fn encode_runtime_binary_operation(
    destination_register: u8,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();

    match operator {
        StateGuardOperator::Add => {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                18,
            ));
        }
        StateGuardOperator::Subtract => {
            bytes.extend(encode_sub_x_register(
                destination_register,
                destination_register,
                18,
            ));
        }
        StateGuardOperator::Equal | StateGuardOperator::NotEqual => {
            bytes.extend(encode_compare_w_register(destination_register, 18));
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
