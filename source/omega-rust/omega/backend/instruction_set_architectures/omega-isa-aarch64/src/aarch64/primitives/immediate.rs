use psi_diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_movz(register: u8, immediate: u16) -> [u8; 4] {
    encode_instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

pub(in crate::aarch64) fn encode_movz_w(register: u8, immediate: u16) -> [u8; 4] {
    encode_instruction(0x52800000 | (u32::from(immediate) << 5) | u32::from(register))
}

pub(in crate::aarch64) fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> [u8; 4] {
    encode_instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

/// `MOVK Wd, #immediate, LSL #(halfword_shift * 16)` — the 32-bit form only
/// supports halfword shifts 0 and 1.
pub(in crate::aarch64) fn encode_movk_w(
    register: u8,
    immediate: u16,
    halfword_shift: u8,
) -> [u8; 4] {
    debug_assert!(halfword_shift < 2, "W-register MOVK only shifts 0 or 16");
    encode_instruction(
        0x72800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

pub(in crate::aarch64) fn encode_adrp_placeholder(register: u8) -> [u8; 4] {
    encode_instruction(0x90000000 | u32::from(register))
}

pub(in crate::aarch64) fn encode_add_page_offset_placeholder(register: u8) -> [u8; 4] {
    encode_instruction(0x91000000 | (u32::from(register) << 5) | u32::from(register))
}

pub(in crate::aarch64) fn append_immediate(
    bytes: &mut Vec<u8>,
    register: u8,
    value: i64,
) -> Result<(), Diagnostic> {
    // Negative values materialize as their full 64-bit two's-complement bit
    // pattern, mirroring the x86_64 backend's `mov reg, imm64`.
    append_unsigned_immediate(bytes, register, value as u64);
    Ok(())
}

pub(in crate::aarch64) fn append_unsigned_immediate(bytes: &mut Vec<u8>, register: u8, value: u64) {
    bytes.extend(encode_movz(register, halfword(value, 0)));

    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }
}

pub(in crate::aarch64) fn append_unsigned_immediate_padded(
    bytes: &mut Vec<u8>,
    register: u8,
    value: u64,
) {
    bytes.extend(encode_movz(register, halfword(value, 0)));

    for halfword_shift in 1..4 {
        bytes.extend(encode_movk(
            register,
            halfword(value, halfword_shift),
            halfword_shift,
        ));
    }
}

/// Materialize a full 32-bit value into a W register as a fixed-width pair of
/// `MOVZ` + `MOVK` halfword moves (always 8 bytes), so widths stay independent
/// of the value's magnitude.
pub(in crate::aarch64) fn append_unsigned_immediate_w_padded(
    bytes: &mut Vec<u8>,
    register: u8,
    value: u32,
) {
    bytes.extend(encode_movz_w(register, (value & 0xffff) as u16));
    bytes.extend(encode_movk_w(register, (value >> 16) as u16, 1));
}

pub(in crate::aarch64) fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
