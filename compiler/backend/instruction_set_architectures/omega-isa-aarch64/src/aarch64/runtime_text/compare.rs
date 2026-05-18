use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_cbz_x,
    encode_compare_w_register, encode_compare_w17_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_byte_w_post_increment,
    encode_load_byte_w17_from_x16, encode_load_x_from_x,
    encode_runtime_text_input_delimiter_check_bytes, encode_subs_x_immediate,
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
) -> Result<[u8; 84], Diagnostic> {
    let mut bytes = [0; 84];
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
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_load_x_from_x(18, 17, source_offset)?,
    );
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_load_x_from_x(19, 17, source_offset + 8)?,
    );

    append_fixed_instruction(&mut bytes, &mut cursor, encode_cbz_x(19, 28)?);
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_load_byte_w_post_increment(20, 18, 1)?,
    );
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_load_byte_w_post_increment(21, 16, 1)?,
    );
    append_fixed_instruction(&mut bytes, &mut cursor, encode_compare_w_register(20, 21));
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        if branch_when_equal {
            encode_conditional_branch_equal(compare_failure_branch_distance)?
        } else {
            encode_conditional_branch_not_equal(compare_failure_branch_distance)?
        },
    );
    append_fixed_instruction(&mut bytes, &mut cursor, encode_subs_x_immediate(19, 19, 1)?);
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_conditional_branch_not_equal(-20)?,
    );
    append_fixed_instructions(
        &mut bytes,
        &mut cursor,
        encode_runtime_text_input_delimiter_check_bytes(0, delimiter_failure_branch_distance)?,
    );
    debug_assert_eq!(cursor, runtime_text_storage_compare_width());
    Ok(bytes)
}

fn append_fixed_instruction<const BYTE_COUNT: usize>(
    bytes: &mut [u8; BYTE_COUNT],
    cursor: &mut usize,
    instruction: [u8; 4],
) {
    bytes[*cursor..*cursor + 4].copy_from_slice(&instruction);
    *cursor += 4;
}

fn append_fixed_instructions<const BYTE_COUNT: usize, const INSTRUCTION_BYTE_COUNT: usize>(
    bytes: &mut [u8; BYTE_COUNT],
    cursor: &mut usize,
    instructions: [u8; INSTRUCTION_BYTE_COUNT],
) {
    bytes[*cursor..*cursor + INSTRUCTION_BYTE_COUNT].copy_from_slice(&instructions);
    *cursor += INSTRUCTION_BYTE_COUNT;
}
