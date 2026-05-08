use omega_core::diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_movz(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

pub(in crate::aarch64) fn encode_movz_w(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0x52800000 | (u32::from(immediate) << 5) | u32::from(register))
}

pub(in crate::aarch64) fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> Vec<u8> {
    encode_instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

pub(in crate::aarch64) fn encode_adrp_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x90000000 | u32::from(register))
}

pub(in crate::aarch64) fn encode_add_page_offset_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x91000000 | (u32::from(register) << 5) | u32::from(register))
}

pub(in crate::aarch64) fn encode_immediate(
    register: u8,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode negative immediate `{value}` yet"
        ))
    })?;

    Ok(encode_unsigned_immediate(register, value))
}

pub(in crate::aarch64) fn encode_unsigned_immediate(register: u8, value: u64) -> Vec<u8> {
    let mut bytes = encode_movz(register, halfword(value, 0));

    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }

    bytes
}

pub(in crate::aarch64) fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
