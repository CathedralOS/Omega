use omega_core::diagnostics::Diagnostic;

use super::append_unsigned_immediate;
use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_add_x_immediate(
    destination_register: u8,
    source_register: u8,
    value: usize,
) -> Result<[u8; 4], Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot add immediate `{value}` yet"
        )));
    }
    Ok(encode_instruction(
        0x91000000
            | ((value as u32) << 10)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    ))
}

pub(in crate::aarch64) fn append_add_x_constant(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    source_register: u8,
    value: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if value == 0 {
        if destination_register != source_register {
            bytes.extend(super::encode_move_x_register(
                destination_register,
                source_register,
            ));
        }
        return Ok(());
    }

    if value <= 4095 {
        bytes.extend(encode_add_x_immediate(
            destination_register,
            source_register,
            value,
        )?);
        return Ok(());
    }

    let immediate_register = if destination_register == source_register {
        scratch_register
    } else {
        destination_register
    };
    append_unsigned_immediate(bytes, immediate_register, value as u64);
    bytes.extend(encode_add_x_register(
        destination_register,
        source_register,
        immediate_register,
    ));
    Ok(())
}

pub(in crate::aarch64) fn encode_add_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x8B000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_and_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x8A000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_orr_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xAA000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_sub_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xCB000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_mul_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9B007C00
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_udiv_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9AC00800
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `LSLV Xd, Xn, Xm` — logical shift left of `left_register` by the low bits of
/// `right_register`. Data-processing (2 source), opcode `0b001000` (`0x08 << 10`).
pub(in crate::aarch64) fn encode_lslv_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9AC02000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `LSRV Xd, Xn, Xm` — LOGICAL shift right (zero-fill), opcode `0b001001`. Used for
/// an unsigned `>>` (`ShiftRightLogical`).
pub(in crate::aarch64) fn encode_lsrv_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9AC02400
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `ASRV Xd, Xn, Xm` — ARITHMETIC shift right (sign-fill), opcode `0b001010`. Used
/// for a signed `>>` (`ShiftRight`).
pub(in crate::aarch64) fn encode_asrv_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9AC02800
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_msub_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
    minuend_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9B008000
            | (u32::from(right_register) << 16)
            | (u32::from(minuend_register) << 10)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `SXTW Xd, Wn` — sign-extend the low 32 bits of `source_register` into the
/// full 64-bit `destination_register` (an `SBFM` alias). Needed when widening a
/// signed 32-bit integer source in a numeric `as` cast.
///
/// Ready for the numeric `as` cast lowering; not yet wired.
#[allow(dead_code)]
pub(in crate::aarch64) fn encode_sign_extend_word_to_x(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9340_7C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_subs_x_immediate(
    destination_register: u8,
    source_register: u8,
    value: usize,
) -> Result<[u8; 4], Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot subtract immediate `{value}` yet"
        )));
    }
    Ok(encode_instruction(
        0xF1000000
            | ((value as u32) << 10)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    ))
}
