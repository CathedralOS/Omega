use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    append_unsigned_immediate, encode_add_page_offset_placeholder, encode_add_x_immediate,
    encode_adrp_placeholder, encode_branch_link_placeholder, encode_cbnz_x, encode_cbz_x,
    encode_compare_w_immediate, encode_conditional_branch_equal,
    encode_conditional_branch_not_equal, encode_load_byte_w_from_x, encode_move_x_register,
    encode_movz, encode_store_byte_w_to_x, encode_store_x_to_x, encode_svc,
    encode_unconditional_branch,
};
use super::super::widths::{
    runtime_text_line_read_import_width, runtime_text_line_read_syscall_width,
};

#[derive(Clone, Copy)]
enum RuntimeTextReadCall {
    Import,
    Syscall {
        number: u32,
        number_register: u8,
        supervisor_call: u16,
    },
}

pub fn encode_runtime_text_line_read_import(
    target_offset: usize,
    byte_capacity: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_line_read(target_offset, byte_capacity, RuntimeTextReadCall::Import)
}

pub fn encode_runtime_text_line_read_syscall(
    target_offset: usize,
    byte_capacity: usize,
    number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_line_read(
        target_offset,
        byte_capacity,
        RuntimeTextReadCall::Syscall {
            number,
            number_register,
            supervisor_call,
        },
    )
}

fn encode_runtime_text_line_read(
    target_offset: usize,
    byte_capacity: usize,
    call: RuntimeTextReadCall,
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
    let encoded_capacity = match call {
        RuntimeTextReadCall::Import => runtime_text_line_read_import_width(byte_capacity),
        RuntimeTextReadCall::Syscall { number, .. } => {
            runtime_text_line_read_syscall_width(byte_capacity, number)
        }
    };
    let mut bytes = Vec::with_capacity(encoded_capacity);
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(21, 20));
    bytes.extend(encode_movz(22, 0));

    let read_loop_offset = bytes.len();
    bytes.extend(encode_movz(0, 0));
    bytes.extend(encode_move_x_register(1, 21));
    bytes.extend(encode_movz(2, 1));
    match call {
        RuntimeTextReadCall::Import => {
            bytes.extend(encode_branch_link_placeholder());
        }
        RuntimeTextReadCall::Syscall {
            number,
            number_register,
            supervisor_call,
        } => {
            append_unsigned_immediate(&mut bytes, number_register, u64::from(number));
            bytes.extend(encode_svc(supervisor_call));
        }
    }
    bytes.extend(encode_cbz_x(0, 64)?);
    bytes.extend(encode_load_byte_w_from_x(24, 21, 0)?);
    // A '\n'/'\r' delimiter terminates the line only once content is present
    // (x22 > 0); a LEADING one is skipped (branch back to the read loop without
    // accepting it), so CRLF acts as a single terminator -- the '\n' trailing a
    // '\r'-ended line never surfaces as a phantom empty line to the next
    // read_line. Mirrors the x86_64 reference loop.
    for (delimiter, finish_line_distance) in [(10, 48isize), (13, 32isize)] {
        bytes.extend(encode_compare_w_immediate(24, delimiter)?);
        bytes.extend(encode_conditional_branch_not_equal(12)?);
        bytes.extend(encode_cbnz_x(22, finish_line_distance)?);
        let skip_leading_delimiter_distance = read_loop_offset as isize - bytes.len() as isize;
        bytes.extend(encode_unconditional_branch(skip_leading_delimiter_distance)?);
    }
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

    let expected_width = match call {
        RuntimeTextReadCall::Import => runtime_text_line_read_import_width(byte_capacity),
        RuntimeTextReadCall::Syscall { number, .. } => {
            runtime_text_line_read_syscall_width(byte_capacity, number)
        }
    };
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}
