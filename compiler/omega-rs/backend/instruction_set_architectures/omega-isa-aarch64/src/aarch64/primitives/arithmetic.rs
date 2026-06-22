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

/// `LDADDAL <Ws/Xs>, WZR/XZR, [<Xn>]` — LSE (ARMv8.1) atomic fetch-add that
/// discards the prior value (Rt = 31 = the zero register). The ACQUIRE+RELEASE
/// variant (A=1, R=1) is chosen to match x86 `LOCK xadd`'s sequentially-
/// consistent barrier: the requested C11 ordering is erased before codegen
/// today, so the strongest ordering is the conservative cross-architecture
/// choice (unobservable without a thread to witness it). `byte_size` selects
/// the size field: 1→LDADDALB, 2→LDADDALH, 4→32-bit, 8→64-bit.
pub(in crate::aarch64) fn encode_ldaddal_discard(
    byte_size: usize,
    add_register: u8,
    address_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let size = match byte_size {
        1 => 0u32,
        2 => 1,
        4 => 2,
        8 => 3,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic fetch_add cannot encode a {other}-byte width"
            )));
        }
    };
    Ok(encode_instruction(
        0x38E0_0000
            | (size << 30)
            | (u32::from(add_register) << 16)
            | (u32::from(address_register) << 5)
            | 31, // Rt = WZR/XZR: discard the returned prior value
    ))
}

/// `CASAL <Ws/Xs>, <Wt/Xt>, [<Xn>]` — LSE (ARMv8.1) compare-and-swap with full
/// (acquire+release) ordering to match x86 `LOCK CMPXCHG`. `Rs` (the compare
/// register) holds `expected` on entry and is OVERWRITTEN with the place's prior
/// value on exit; `Rt` is the `new_value` stored only when the compare matched.
/// Base opcode `0x08E0_FC00` (L=1, o0=1, Rt2=11111); `byte_size` selects the size
/// field: 1→CASALB, 2→CASALH, 4→32-bit, 8→64-bit.
pub(in crate::aarch64) fn encode_casal(
    byte_size: usize,
    compare_register: u8,
    new_value_register: u8,
    address_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let size = match byte_size {
        1 => 0u32,
        2 => 1,
        4 => 2,
        8 => 3,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic compare_exchange cannot encode a {other}-byte width"
            )));
        }
    };
    Ok(encode_instruction(
        0x08E0_FC00
            | (size << 30)
            | (u32::from(compare_register) << 16)
            | (u32::from(address_register) << 5)
            | u32::from(new_value_register),
    ))
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

/// `UDIV Wd, Wn, Wm` — 32-bit unsigned divide, for operands narrower than 8 bytes.
pub(in crate::aarch64) fn encode_udiv_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1AC00800
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `SDIV Xd, Xn, Xm` — 64-bit signed divide (data-processing 2-source, opcode
/// `0b000011`).
pub(in crate::aarch64) fn encode_sdiv_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9AC00C00
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `SDIV Wd, Wn, Wm` — 32-bit signed divide, so an i32 sign bit is honored when
/// the operands were loaded zero-extended into 64-bit registers.
pub(in crate::aarch64) fn encode_sdiv_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1AC00C00
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

/// `ASRV Wd, Wn, Wm` — 32-bit arithmetic shift right, so an i32 sign bit is
/// sign-filled correctly when the operand was loaded zero-extended.
pub(in crate::aarch64) fn encode_asrv_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1AC02800
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

/// `MSUB Wd, Wn, Wm, Wa` — 32-bit multiply-subtract (`Wd = Wa - Wn*Wm`), used to
/// derive a 32-bit remainder from a 32-bit quotient.
pub(in crate::aarch64) fn encode_msub_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
    minuend_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1B008000
            | (u32::from(right_register) << 16)
            | (u32::from(minuend_register) << 10)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `SXTB Wd, Wn` — sign-extend the low byte into the 32-bit register (`SBFM`
/// alias). Used to compare narrow signed operands at the 32-bit width.
pub(in crate::aarch64) fn encode_sign_extend_byte_to_w(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1300_1C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `SXTH Wd, Wn` — sign-extend the low halfword into the 32-bit register (`SBFM`
/// alias). Used to compare narrow signed operands at the 32-bit width.
pub(in crate::aarch64) fn encode_sign_extend_halfword_to_w(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1300_3C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
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

/// `SXTB Xd, Wn` — sign-extend the low byte of `source_register` into the full
/// 64-bit `destination_register` (`SBFM Xd, Xn, #0, #7`). Used to widen a signed
/// 8-bit operand to 64 bits before a saturating/trapping wide-width op so the
/// sign bit is honored (the operand was loaded zero-extended).
pub(in crate::aarch64) fn encode_sign_extend_byte_to_x(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9340_1C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `SXTH Xd, Wn` — sign-extend the low halfword of `source_register` into the
/// full 64-bit `destination_register` (`SBFM Xd, Xn, #0, #15`). Used to widen a
/// signed 16-bit operand to 64 bits before a saturating/trapping wide-width op.
pub(in crate::aarch64) fn encode_sign_extend_halfword_to_x(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9340_3C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
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

/// `EOR Xd, Xn, Xm` — bitwise exclusive or (shifted register, shift 0).
pub(in crate::aarch64) fn encode_eor_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xCA000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `AND Xd, Xn, #0x7f` — keep the low seven bits (one LEB128 payload group).
/// Logical immediate with element size 64: N=1, immr=0, imms=6.
pub(in crate::aarch64) fn encode_and_x_immediate_low_seven(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9240_1800 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `ORR Xd, Xn, #0x80` — set the LEB128 continuation bit. Logical immediate
/// with element size 64: N=1, immr=57 (a single one rotated up to bit 7),
/// imms=0.
pub(in crate::aarch64) fn encode_orr_x_immediate_bit_seven(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xB279_0000 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `LSR Xd, Xn, #shift` — logical shift right by a constant (`UBFM Xd, Xn,
/// #shift, #63`).
pub(in crate::aarch64) fn encode_lsr_x_immediate(
    destination_register: u8,
    source_register: u8,
    shift: u8,
) -> [u8; 4] {
    encode_instruction(
        0xD340_FC00
            | (u32::from(shift & 0x3f) << 16)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    )
}

/// `ASR Xd, Xn, #shift` — arithmetic shift right by a constant (`SBFM Xd, Xn,
/// #shift, #63`).
pub(in crate::aarch64) fn encode_asr_x_immediate(
    destination_register: u8,
    source_register: u8,
    shift: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9340_FC00
            | (u32::from(shift & 0x3f) << 16)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    )
}
