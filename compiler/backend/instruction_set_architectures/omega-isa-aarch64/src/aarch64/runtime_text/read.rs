use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_adrp_placeholder,
    encode_cbz_x, encode_compare_w_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_byte_w_from_x, encode_move_x_register,
    encode_movz, encode_store_byte_w_to_x, encode_store_x_to_x, encode_svc,
    encode_unsigned_immediate,
};
use super::super::widths::runtime_text_line_read_width;

pub fn encode_runtime_text_line_read(
    target_offset: usize,
    byte_capacity: usize,
    syscall_number: u32,
    syscall_number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let max_payload_bytes = byte_capacity.saturating_sub(1);
    let capacity = u32::try_from(max_payload_bytes).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 runtime line read cannot encode capacity `{byte_capacity}` yet"
        ))
    })?;
    if capacity > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 runtime line read cannot compare capacity `{byte_capacity}` yet"
        )));
    }
    let mut bytes = Vec::new();
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(21, 20));
    bytes.extend(encode_movz(22, 0));

    let read_loop_offset = bytes.len();
    bytes.extend(encode_movz(0, 0));
    bytes.extend(encode_move_x_register(1, 21));
    bytes.extend(encode_movz(2, 1));
    bytes.extend(encode_unsigned_immediate(
        syscall_number_register,
        u64::from(syscall_number),
    ));
    bytes.extend(encode_svc(supervisor_call));
    bytes.extend(encode_cbz_x(0, 48)?);
    bytes.extend(encode_load_byte_w_from_x(24, 21, 0)?);
    bytes.extend(encode_compare_w_immediate(24, 10)?);
    bytes.extend(encode_conditional_branch_equal(36)?);
    bytes.extend(encode_compare_w_immediate(24, 13)?);
    bytes.extend(encode_conditional_branch_equal(28)?);
    bytes.extend(encode_compare_w_immediate(24, 0)?);
    bytes.extend(encode_conditional_branch_equal(20)?);
    bytes.extend(encode_add_x_immediate(21, 21, 1)?);
    bytes.extend(encode_add_x_immediate(22, 22, 1)?);
    bytes.extend(encode_compare_w_immediate(22, capacity)?);
    let repeat_read_distance = read_loop_offset as isize - bytes.len() as isize;
    bytes.extend(encode_conditional_branch_not_equal(repeat_read_distance)?);

    bytes.extend(encode_store_byte_w_to_x(31, 21, 0)?);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x_to_x(20, 16, target_offset)?);
    bytes.extend(encode_store_x_to_x(22, 16, target_offset + 8)?);

    debug_assert_eq!(
        bytes.len(),
        runtime_text_line_read_width(byte_capacity, syscall_number)
    );
    Ok(bytes)
}
