use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_cbz_x,
    encode_compare_w_register, encode_compare_w17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_byte_w_post_increment,
    encode_load_byte_w17_from_x16, encode_load_x_from_x, encode_runtime_text_input_delimiter_check,
    encode_subs_x_immediate,
};

pub fn encode_runtime_text_literal_compare(
    literal: &str,
    failure_branch_distances: Vec<isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    if literal.len() != failure_branch_distances.len() {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime text guard expected {} branch distance(s), got {}",
            literal.len(),
            failure_branch_distances.len()
        )));
    }

    let mut bytes = Vec::with_capacity(32);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, expected_byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_load_byte_w17_from_x16(byte_index)?);
        bytes.extend(encode_compare_w17_immediate(u32::from(*expected_byte))?);
        bytes.extend(encode_conditional_branch_not_equal(
            failure_branch_distances[byte_index],
        )?);
    }

    bytes.extend(encode_runtime_text_input_delimiter_check(
        literal.len(),
        delimiter_failure_branch_distance,
    )?);
    Ok(bytes)
}

pub fn encode_runtime_text_storage_compare(
    source_offset: usize,
    compare_failure_branch_distance: isize,
    delimiter_failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(18, 17, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 17, source_offset + 8)?);

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
    bytes.extend(encode_runtime_text_input_delimiter_check(
        0,
        delimiter_failure_branch_distance,
    )?);
    Ok(bytes)
}
