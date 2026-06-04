use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    append_add_x_constant, encode_add_page_offset_placeholder, encode_adrp_placeholder,
    encode_cbz_x, encode_compare_w_register, encode_compare_w17_immediate,
    encode_conditional_branch_equal, encode_conditional_branch_not_equal,
    encode_load_byte_w_post_increment, encode_load_byte_w17_from_x16, encode_load_x_from_x,
    encode_move_x_register, encode_runtime_text_input_delimiter_check_bytes,
    encode_subs_x_immediate,
};
use super::super::widths::{
    runtime_text_literal_compare_width, runtime_text_storage_compare_width,
};

pub fn encode_runtime_text_literal_compare(
    literal: &str,
    failure_branch_distances: impl ExactSizeIterator<Item = isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let failure_branch_distance_count = failure_branch_distances.len();
    if literal.len() != failure_branch_distance_count {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime text guard expected {} branch distance(s), got {}",
            literal.len(),
            failure_branch_distance_count
        )));
    }

    let mut bytes = Vec::with_capacity(runtime_text_literal_compare_width(literal));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, (expected_byte, failure_branch_distance)) in literal
        .as_bytes()
        .iter()
        .zip(failure_branch_distances)
        .enumerate()
    {
        bytes.extend(encode_load_byte_w17_from_x16(byte_index)?);
        bytes.extend(encode_compare_w17_immediate(u32::from(*expected_byte))?);
        bytes.extend(encode_conditional_branch_not_equal(
            failure_branch_distance,
        )?);
    }

    bytes.extend(encode_runtime_text_input_delimiter_check_bytes(
        literal.len(),
        delimiter_failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_text_storage_compare_bytes(
    source_offset: usize,
    compare_failure_branch_distance: isize,
    delimiter_failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_storage_compare_width(source_offset));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 18, 17, source_offset, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 17, source_offset + 8, 15)?;

    bytes.extend(encode_cbz_x(19, 28)?);
    bytes.extend(encode_load_byte_w_post_increment(20, 18, 1)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 16, 1)?);
    bytes.extend(encode_compare_w_register(20, 21));
    bytes.extend(if branch_when_equal {
        encode_conditional_branch_equal(compare_failure_branch_distance)?
    } else {
        encode_conditional_branch_not_equal(compare_failure_branch_distance)?
    });
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-20)?);
    bytes.extend(encode_runtime_text_input_delimiter_check_bytes(
        0,
        delimiter_failure_branch_distance,
    )?);
    debug_assert_eq!(
        bytes.len(),
        runtime_text_storage_compare_width(source_offset)
    );
    Ok(bytes)
}

fn append_load_x_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, 8) {
        bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            byte_offset,
        )?);
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_x_constant(bytes, scratch_register, scratch_register, byte_offset, 14)?;
        bytes.extend(encode_load_x_from_x(
            destination_register,
            scratch_register,
            0,
        )?);
    }

    Ok(())
}

fn data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}
