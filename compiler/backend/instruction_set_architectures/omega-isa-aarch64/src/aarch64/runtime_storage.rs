use omega_core::diagnostics::Diagnostic;

use super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_compare_w_register,
    encode_compare_w17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_w_from_x, encode_load_x_from_x, encode_movz_w,
    encode_store_w_to_x, encode_store_w17_to_x16, encode_store_x_to_x, encode_store_x17_to_x16,
    encode_unsigned_immediate, encode_unsigned_immediate_padded,
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
