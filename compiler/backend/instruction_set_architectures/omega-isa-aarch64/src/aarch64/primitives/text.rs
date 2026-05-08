use omega_core::diagnostics::Diagnostic;

use super::branch::{encode_conditional_branch_equal, encode_unconditional_branch};
use super::compare::encode_compare_w17_immediate;
use super::memory::encode_load_byte_w17_from_x16;

pub(in crate::aarch64) fn encode_runtime_text_input_delimiter_check(
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_load_byte_w17_from_x16(byte_offset)?;
    bytes.extend(encode_compare_w17_immediate(10)?);
    bytes.extend(encode_conditional_branch_equal(24)?);
    bytes.extend(encode_compare_w17_immediate(13)?);
    bytes.extend(encode_conditional_branch_equal(16)?);
    bytes.extend(encode_compare_w17_immediate(0)?);
    bytes.extend(encode_conditional_branch_equal(8)?);
    bytes.extend(encode_unconditional_branch(failure_branch_distance)?);
    Ok(bytes)
}
