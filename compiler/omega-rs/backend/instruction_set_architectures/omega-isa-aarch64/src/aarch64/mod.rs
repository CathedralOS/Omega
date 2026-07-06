use crate::Aarch64CallOperand;
use crate::Aarch64CallOperand::*;
use omega_core::diagnostics::Diagnostic;

mod dispatch;
mod primitives;
mod runtime_storage;
mod runtime_text;
mod widths;
mod wire_decode;
mod wire_encode;

pub use dispatch::*;
use primitives::*;
pub use runtime_storage::*;
pub use runtime_text::*;
pub use widths::*;
pub use wire_decode::*;
pub use wire_encode::*;

pub fn encode_host_call_sequence(operands: &[Aarch64CallOperand]) -> Result<Vec<u8>, Diagnostic> {
    encode_host_call_sequence_from_operands(operands.iter().copied())
}

pub fn encode_host_call_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(host_call_sequence_width_from_operands(operands.clone()));
    append_call_operands(&mut bytes, operands)?;
    bytes.extend(encode_branch_link_placeholder());
    Ok(bytes)
}

/// A VALUE-RETURNING host call: `operands[0]` is the result storage place, the
/// rest are the call arguments. Marshal the args into x0.., branch-link to the
/// callee (relocated to the import symbol), then store the return register into
/// the result place — `str w0` for a 4-byte result (an `i32` fd/rc; the sign is
/// preserved so a negative `-errno` reads back correctly), `str x0` for 8 bytes
/// (an `i64` byte count). Mirrors x86_64's `encode_win64_import_call(.., true)`.
/// The total width equals the non-returning form: the result operand's scalar
/// width (adrp+add+ldr = 12) is the same as its store (adrp+add+str = 12).
pub fn encode_host_call_sequence_value_returning_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
) -> Result<Vec<u8>, Diagnostic> {
    let all: Vec<Aarch64CallOperand> = operands.collect();
    let Some((result, args)) = all.split_first() else {
        return Err(Diagnostic::error(
            "AArch64 value-returning host call has no result storage operand",
        ));
    };
    let RuntimeScalarInteger {
        byte_offset,
        byte_count,
    } = *result
    else {
        return Err(Diagnostic::error(
            "AArch64 value-returning host call result place did not lower to a runtime scalar",
        ));
    };
    let mut bytes = Vec::with_capacity(host_call_sequence_width_from_operands(all.iter().copied()));
    append_call_operands(&mut bytes, args.iter().copied())?;
    bytes.extend(encode_branch_link_placeholder());
    // Result store: x16 <- result region base (adrp/add relocated), then store
    // the return register at the field's offset.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if byte_count >= 8 {
        bytes.extend(encode_store_x_to_x(0, 16, byte_offset)?);
    } else {
        bytes.extend(encode_store_w_to_x(0, 16, byte_offset, byte_count)?);
    }
    Ok(bytes)
}

pub fn encode_syscall_sequence(
    operands: &[Aarch64CallOperand],
    syscall_number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    encode_syscall_sequence_from_operands(
        operands.iter().copied(),
        syscall_number,
        number_register,
        supervisor_call,
    )
}

pub fn encode_syscall_sequence_from_operands(
    operands: impl Iterator<Item = Aarch64CallOperand> + Clone,
    syscall_number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(syscall_sequence_width_from_operands(
        operands.clone(),
        syscall_number,
    ));
    append_call_operands(&mut bytes, operands)?;
    append_unsigned_immediate(&mut bytes, number_register, u64::from(syscall_number));
    bytes.extend(encode_svc(supervisor_call));
    Ok(bytes)
}

pub fn encode_function_enter_bytes() -> [u8; 28] {
    let mut bytes = [0; 28];
    bytes[0..4].copy_from_slice(&encode_instruction(0xA9BA7BFD));
    bytes[4..8].copy_from_slice(&encode_instruction(0x910003FD));
    bytes[8..12].copy_from_slice(&encode_instruction(0xA90153F3));
    bytes[12..16].copy_from_slice(&encode_instruction(0xA9025BF5));
    bytes[16..20].copy_from_slice(&encode_instruction(0xA90363F7));
    bytes[20..24].copy_from_slice(&encode_instruction(0xA9046BF9));
    bytes[24..28].copy_from_slice(&encode_instruction(0xA90573FB));
    bytes
}

pub fn encode_return_bytes() -> [u8; 28] {
    let mut bytes = [0; 28];
    bytes[0..4].copy_from_slice(&encode_instruction(0xA94153F3));
    bytes[4..8].copy_from_slice(&encode_instruction(0xA9425BF5));
    bytes[8..12].copy_from_slice(&encode_instruction(0xA94363F7));
    bytes[12..16].copy_from_slice(&encode_instruction(0xA9446BF9));
    bytes[16..20].copy_from_slice(&encode_instruction(0xA94573FB));
    bytes[20..24].copy_from_slice(&encode_instruction(0xA8C67BFD));
    bytes[24..28].copy_from_slice(&encode_instruction(0xD65F03C0));
    bytes
}

pub fn encode_return_register_integer_write_bytes(
    byte_size: usize,
    value: i64,
) -> Result<[u8; 4], Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte return integers yet"
        )));
    }
    let immediate = u16::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write return integer `{value}` yet"
        ))
    })?;
    Ok(if byte_size == 8 {
        encode_movz(0, immediate)
    } else {
        encode_movz_w(0, immediate)
    })
}

fn append_call_operands(
    bytes: &mut Vec<u8>,
    operands: impl Iterator<Item = Aarch64CallOperand>,
) -> Result<(), Diagnostic> {
    let mut next_register = 0u8;

    for operand in operands {
        match &operand {
            ImmediateInteger(value) => {
                append_immediate(bytes, next_register, *value)?;
                next_register += 1;
            }
            DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            RuntimeStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            RuntimeStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    byte_offset + 8,
                )?);
                next_register += 1;
            }
            RuntimePointeeStringPointer { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                bytes.extend(encode_load_x_from_x(next_register, next_register, 0)?);
                next_register += 1;
            }
            RuntimePointeeStringLength { byte_offset } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                bytes.extend(encode_load_x_from_x(next_register, next_register, 8)?);
                next_register += 1;
            }
            RuntimeScalarInteger { byte_offset, .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_load_x_from_x(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            RuntimeStorageAddress { byte_offset } => {
                // The place's ADDRESS: adrp/add to the region base (relocated),
                // then add the field offset. No load — the pointer is the arg.
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                bytes.extend(encode_add_x_immediate(
                    next_register,
                    next_register,
                    *byte_offset,
                )?);
                next_register += 1;
            }
            ByteLength(value) => {
                append_unsigned_immediate(bytes, next_register, *value as u64);
                next_register += 1;
            }
        }
    }

    Ok(())
}
