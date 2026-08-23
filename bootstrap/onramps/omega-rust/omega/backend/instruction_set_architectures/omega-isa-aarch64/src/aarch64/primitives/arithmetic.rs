use psi_diagnostics::Diagnostic;

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

pub(in crate::aarch64) fn encode_sub_x_immediate(
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
        0xD1000000
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

/// `ADDS Xd, Xn, Xm` -- flag-setting add (C = unsigned carry, V = signed
/// overflow), the 64-bit overflow-detection workhorse.
/// `SMULH Xd, Xn, Xm` -- high 64 bits of the signed 128-bit product. A 64-bit
/// signed multiply overflowed iff this differs from the low half's sign
/// broadcast (`low >> 63`).
pub(in crate::aarch64) fn encode_smulh_x(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9B407C00
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `UMULH Xd, Xn, Xm` -- high 64 bits of the unsigned 128-bit product. A
/// 64-bit unsigned multiply overflowed iff this is non-zero.
pub(in crate::aarch64) fn encode_umulh_x(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x9BC07C00
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `CMP Xn, Xm, ASR #63` (SUBS XZR, shifted register) -- compare against the
/// other operand's sign broadcast, the signed-multiply overflow test.
pub(in crate::aarch64) fn encode_compare_x_register_sign_broadcast(
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xEB000000
            | (0b10 << 22) // ASR
            | (u32::from(right_register) << 16)
            | (63 << 10)
            | (u32::from(left_register) << 5)
            | 31,
    )
}

pub(in crate::aarch64) fn encode_adds_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xAB000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `SUBS Xd, Xn, Xm` -- flag-setting subtract (C clear = borrow, V = signed
/// overflow). `CMP` is this with Xd = XZR.
pub(in crate::aarch64) fn encode_subs_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xEB000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `CSEL Xd, Xn, Xm, <cond>` -- Xn when the condition holds, else Xm.
pub(in crate::aarch64) fn encode_csel_x(
    destination_register: u8,
    true_register: u8,
    false_register: u8,
    condition: u32,
) -> [u8; 4] {
    encode_instruction(
        0x9A800000
            | (u32::from(false_register) << 16)
            | (condition << 12)
            | (u32::from(true_register) << 5)
            | u32::from(destination_register),
    )
}

/// `CSINV Xd, Xn, Xm, <cond>` -- Xn when the condition holds, else NOT(Xm).
/// `csinv Xd, Xm, Xm, cond` therefore picks a bound or its complement in one
/// instruction (i64::MIN vs i64::MAX), and `csinv Xd, Xn, XZR, cond` yields
/// all-ones (u64::MAX) on the else-path.
pub(in crate::aarch64) fn encode_csinv_x(
    destination_register: u8,
    true_register: u8,
    false_register: u8,
    condition: u32,
) -> [u8; 4] {
    encode_instruction(
        0xDA800000
            | (u32::from(false_register) << 16)
            | (condition << 12)
            | (u32::from(true_register) << 5)
            | u32::from(destination_register),
    )
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

/// `LDADD{A}{L} <Ws/Xs>, <Wt/Xt>, [<Xn>]`: LSE atomic fetch-add.
/// The requested ordering selects the A/R bits and Rt receives the prior value.
pub(in crate::aarch64) fn encode_ldadd(
    byte_size: usize,
    add_register: u8,
    result_register: u8,
    address_register: u8,
    ordering: psi_language_core::MemoryOrdering,
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
    let ordering_bits = match ordering {
        psi_language_core::MemoryOrdering::NoOrdering => 0,
        psi_language_core::MemoryOrdering::Receive => 0x0080_0000,
        psi_language_core::MemoryOrdering::Publish => 0x0040_0000,
        psi_language_core::MemoryOrdering::ReceivePublish
        | psi_language_core::MemoryOrdering::GlobalOrder => 0x00C0_0000,
    };
    Ok(encode_instruction(
        0x3820_0000
            | (size << 30)
            | ordering_bits
            | (u32::from(add_register) << 16)
            | (u32::from(address_register) << 5)
            | u32::from(result_register),
    ))
}

/// `LDEOR{A}{L} <Ws/Xs>, <Wt/Xt>, [<Xn>]`: LSE atomic fetch-XOR.
/// The requested ordering selects the A/R bits and Rt receives the prior value.
pub(in crate::aarch64) fn encode_ldeor(
    byte_size: usize,
    value_register: u8,
    result_register: u8,
    address_register: u8,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<[u8; 4], Diagnostic> {
    let size = match byte_size {
        1 => 0u32,
        2 => 1,
        4 => 2,
        8 => 3,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic fetch_xor cannot encode a {other}-byte width"
            )));
        }
    };
    let ordering_bits = match ordering {
        psi_language_core::MemoryOrdering::NoOrdering => 0,
        psi_language_core::MemoryOrdering::Receive => 0x0080_0000,
        psi_language_core::MemoryOrdering::Publish => 0x0040_0000,
        psi_language_core::MemoryOrdering::ReceivePublish
        | psi_language_core::MemoryOrdering::GlobalOrder => 0x00C0_0000,
    };
    Ok(encode_instruction(
        0x3820_2000
            | (size << 30)
            | ordering_bits
            | (u32::from(value_register) << 16)
            | (u32::from(address_register) << 5)
            | u32::from(result_register),
    ))
}

/// `LDSET{A}{L} <Ws/Xs>, <Wt/Xt>, [<Xn>]`: LSE atomic fetch-OR.
/// The requested ordering selects the A/R bits and Rt receives the prior value.
pub(in crate::aarch64) fn encode_ldset(
    byte_size: usize,
    value_register: u8,
    result_register: u8,
    address_register: u8,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<[u8; 4], Diagnostic> {
    let size = match byte_size {
        1 => 0u32,
        2 => 1,
        4 => 2,
        8 => 3,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic fetch_or cannot encode a {other}-byte width"
            )));
        }
    };
    let ordering_bits = match ordering {
        psi_language_core::MemoryOrdering::NoOrdering => 0,
        psi_language_core::MemoryOrdering::Receive => 0x0080_0000,
        psi_language_core::MemoryOrdering::Publish => 0x0040_0000,
        psi_language_core::MemoryOrdering::ReceivePublish
        | psi_language_core::MemoryOrdering::GlobalOrder => 0x00C0_0000,
    };
    Ok(encode_instruction(
        0x3820_3000
            | (size << 30)
            | ordering_bits
            | (u32::from(value_register) << 16)
            | (u32::from(address_register) << 5)
            | u32::from(result_register),
    ))
}

/// `LDCLR{A}{L} <Ws/Xs>, <Wt/Xt>, [<Xn>]`: LSE atomic fetch-clear.
/// Supplying the complement of an AND mask realizes atomic fetch-AND.
pub(in crate::aarch64) fn encode_ldclr(
    byte_size: usize,
    value_register: u8,
    result_register: u8,
    address_register: u8,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<[u8; 4], Diagnostic> {
    let size = match byte_size {
        1 => 0u32,
        2 => 1,
        4 => 2,
        8 => 3,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic fetch_and cannot encode a {other}-byte width"
            )));
        }
    };
    let ordering_bits = match ordering {
        psi_language_core::MemoryOrdering::NoOrdering => 0,
        psi_language_core::MemoryOrdering::Receive => 0x0080_0000,
        psi_language_core::MemoryOrdering::Publish => 0x0040_0000,
        psi_language_core::MemoryOrdering::ReceivePublish
        | psi_language_core::MemoryOrdering::GlobalOrder => 0x00C0_0000,
    };
    Ok(encode_instruction(
        0x3820_1000
            | (size << 30)
            | ordering_bits
            | (u32::from(value_register) << 16)
            | (u32::from(address_register) << 5)
            | u32::from(result_register),
    ))
}

/// `MVN <Wd/Xd>, <Wm/Xm>` (`ORN` with the zero register).
pub(in crate::aarch64) fn encode_mvn_register(
    byte_size: usize,
    destination_register: u8,
    source_register: u8,
) -> Result<[u8; 4], Diagnostic> {
    let base = match byte_size {
        1 | 2 | 4 => 0x2A20_03E0,
        8 => 0xAA20_03E0,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVN cannot encode a {other}-byte atomic operand"
            )));
        }
    };
    Ok(encode_instruction(
        base | (u32::from(source_register) << 16) | u32::from(destination_register),
    ))
}

/// `SWP{A}{L} <Ws/Xs>, <Wt/Xt>, [<Xn>]`: LSE atomic exchange. The
/// replacement arrives in Rs and Rt receives the instruction-observed prior.
pub(in crate::aarch64) fn encode_swp(
    byte_size: usize,
    replacement_register: u8,
    result_register: u8,
    address_register: u8,
    ordering: psi_language_core::MemoryOrdering,
) -> Result<[u8; 4], Diagnostic> {
    let size = match byte_size {
        1 => 0u32,
        2 => 1,
        4 => 2,
        8 => 3,
        other => {
            return Err(Diagnostic::error(format!(
                "AArch64 atomic swap cannot encode a {other}-byte width"
            )));
        }
    };
    let ordering_bits = match ordering {
        psi_language_core::MemoryOrdering::NoOrdering => 0,
        psi_language_core::MemoryOrdering::Receive => 0x0080_0000,
        psi_language_core::MemoryOrdering::Publish => 0x0040_0000,
        psi_language_core::MemoryOrdering::ReceivePublish
        | psi_language_core::MemoryOrdering::GlobalOrder => 0x00C0_0000,
    };
    Ok(encode_instruction(
        0x3820_8000
            | (size << 30)
            | ordering_bits
            | (u32::from(replacement_register) << 16)
            | (u32::from(address_register) << 5)
            | u32::from(result_register),
    ))
}

/// `CAS{A}{L} <Ws/Xs>, <Wt/Xt>, [<Xn>]`: LSE compare-and-swap.
/// Rs holds expected on entry and receives the prior value on exit; Rt is the
/// new value. The requested success ordering selects the acquire/release form.
pub(in crate::aarch64) fn encode_cas(
    byte_size: usize,
    compare_register: u8,
    new_value_register: u8,
    address_register: u8,
    ordering: psi_language_core::MemoryOrdering,
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
    let ordering_bits = match ordering {
        psi_language_core::MemoryOrdering::NoOrdering => 0,
        psi_language_core::MemoryOrdering::Receive => 0x0040_0000,
        psi_language_core::MemoryOrdering::Publish => 0x0000_8000,
        psi_language_core::MemoryOrdering::ReceivePublish
        | psi_language_core::MemoryOrdering::GlobalOrder => 0x0040_8000,
    };
    Ok(encode_instruction(
        0x08A0_7C00
            | (size << 30)
            | ordering_bits
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

pub(in crate::aarch64) fn encode_sub_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x4B000000
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

/// `LSLV Wd, Wn, Wm` — 32-bit logical shift left. The W form masks the count
/// mod 32, which IS the F8 Wrapping masked-count semantics for 4-byte (and,
/// with the explicit sub-word count mask, 1/2-byte) operands; the X form's
/// mod-64 masking would let a count in [32, 63] compute wide and truncate to
/// the RETIRED modular-value semantics instead.
pub(in crate::aarch64) fn encode_lslv_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1AC02000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

/// `AND Wd, Wn, #((1 << ones) - 1)` — 32-bit bitmask-immediate AND keeping the
/// low `ones` bits (2..=31). Bitmask-immediate encoding for a 32-bit element:
/// `N = 0`, `immr = 0` (no rotation), `imms = ones - 1` (a run of `ones` set
/// bits from bit 0). The F8 Wrapping shift-count mask for sub-word operands
/// (`count & 7` / `count & 15`); the 32/64-bit widths ride the register-form
/// shifts' own masking instead.
pub(in crate::aarch64) fn encode_and_w_low_ones(
    destination_register: u8,
    source_register: u8,
    ones: u32,
) -> [u8; 4] {
    debug_assert!((2..=31).contains(&ones), "low-ones mask needs 2..=31 bits");
    encode_instruction(
        0x12000000
            | ((ones - 1) << 10)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    )
}

/// `AND Xd, Xn, #((1 << ones) - 1)` — 64-bit bitmask-immediate AND keeping the
/// low `ones` bits (2..=63). `N = 1` (64-bit element), `immr = 0`,
/// `imms = ones - 1`. The F5 float-policy guard's ABS mask (low 63 ones
/// clears the sign bit of an f64 bit pattern).
pub(in crate::aarch64) fn encode_and_x_low_ones(
    destination_register: u8,
    source_register: u8,
    ones: u32,
) -> [u8; 4] {
    debug_assert!((2..=63).contains(&ones), "low-ones mask needs 2..=63 bits");
    encode_instruction(
        0x92400000
            | ((ones - 1) << 10)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    )
}

/// `AND Xd, Xn, #0x8000_0000_0000_0000` — keep only the TOP bit (an f64 bit
/// pattern's sign). Bitmask immediate: one set bit (`imms = 0`) rotated
/// right by 1 (`immr = 1`) lands it at bit 63; `N = 1`.
pub(in crate::aarch64) fn encode_and_x_top_bit(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x92400000
            | (1 << 16)
            | (u32::from(source_register) << 5)
            | u32::from(destination_register),
    )
}

/// `AND Wd, Wn, #0x8000_0000` — keep only the top bit of the low word (an
/// f32 bit pattern's sign). `N = 0`, `imms = 0`, `immr = 1`.
pub(in crate::aarch64) fn encode_and_w_top_bit(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x12000000
            | (1 << 16)
            | (u32::from(source_register) << 5)
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

/// `LSRV Wd, Wn, Wm` — 32-bit LOGICAL shift right (zero-fill from bit 31). The
/// narrow form of `encode_lsrv_x_register`: an unsigned `>>` at operand width
/// <= 4 must zero-fill from the OPERAND's width, not bit 63 -- the X form lets
/// garbage/wrapped high bits (e.g. a 64-bit nested Wrapping op's untruncated
/// result) shift down into the live word (the const_fold_unsigned_shift_right
/// arg-delivery face).
pub(in crate::aarch64) fn encode_lsrv_w_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x1AC02400
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
/// `UXTB Wd, Wn` (UBFM #0,#7) -- zero-extend the low byte within a W register.
pub(in crate::aarch64) fn encode_zero_extend_byte_to_w(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x53001C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

/// `UXTH Wd, Wn` (UBFM #0,#15) -- zero-extend the low halfword within a W register.
pub(in crate::aarch64) fn encode_zero_extend_halfword_to_w(
    destination_register: u8,
    source_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x53003C00 | (u32::from(source_register) << 5) | u32::from(destination_register),
    )
}

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

/// `LSL Xd, Xn, #shift`, the `UBFM` alias.
pub(in crate::aarch64) fn encode_lsl_x_immediate(
    destination_register: u8,
    source_register: u8,
    shift: u8,
) -> [u8; 4] {
    debug_assert!((1..64).contains(&shift));
    let immr = 64_u32 - u32::from(shift);
    let imms = 63_u32 - u32::from(shift);
    encode_instruction(
        0xD340_0000
            | (immr << 16)
            | (imms << 10)
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
