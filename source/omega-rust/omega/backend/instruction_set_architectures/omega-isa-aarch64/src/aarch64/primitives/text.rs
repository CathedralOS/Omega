use psi_diagnostics::Diagnostic;

use super::branch::{encode_conditional_branch_equal, encode_unconditional_branch};
use super::compare::encode_compare_w17_immediate;
use super::memory::encode_load_byte_w17_from_x16;

pub(in crate::aarch64) fn encode_runtime_text_input_delimiter_check_bytes(
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<[u8; 32], Diagnostic> {
    let mut bytes = [0u8; 32];
    let mut cursor = 0;
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_load_byte_w17_from_x16(byte_offset)?,
    );
    append_fixed_instruction(&mut bytes, &mut cursor, encode_compare_w17_immediate(10)?);
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_conditional_branch_equal(24)?,
    );
    append_fixed_instruction(&mut bytes, &mut cursor, encode_compare_w17_immediate(13)?);
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_conditional_branch_equal(16)?,
    );
    append_fixed_instruction(&mut bytes, &mut cursor, encode_compare_w17_immediate(0)?);
    append_fixed_instruction(&mut bytes, &mut cursor, encode_conditional_branch_equal(8)?);
    append_fixed_instruction(
        &mut bytes,
        &mut cursor,
        encode_unconditional_branch(failure_branch_distance)?,
    );
    Ok(bytes)
}

fn append_fixed_instruction(bytes: &mut [u8; 32], cursor: &mut usize, instruction: [u8; 4]) {
    let next_cursor = *cursor + instruction.len();
    bytes[*cursor..next_cursor].copy_from_slice(&instruction);
    *cursor = next_cursor;
}
