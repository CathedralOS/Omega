use psi_diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_conditional_branch_not_equal(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.ne")?;
    Ok(encode_instruction(
        0x54000001 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_conditional_branch_equal(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.eq")?;
    Ok(encode_instruction(
        0x54000000 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_conditional_branch_greater(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.gt")?;
    Ok(encode_instruction(
        0x5400000C | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_conditional_branch_greater_or_equal(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.ge")?;
    Ok(encode_instruction(
        0x5400000A | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_conditional_branch_less(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.lt")?;
    Ok(encode_instruction(
        0x5400000B | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_conditional_branch_less_or_equal(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.le")?;
    Ok(encode_instruction(
        0x5400000D | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

/// `b.hs` (a.k.a. `b.cs`, cond `0b0010`) — branch if unsigned higher-or-same.
pub(in crate::aarch64) fn encode_conditional_branch_higher_or_same(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.hs")?;
    Ok(encode_instruction(
        0x54000002 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

/// `b.lo` (a.k.a. `b.cc`, cond `0b0011`) — branch if unsigned lower.
pub(in crate::aarch64) fn encode_conditional_branch_lower(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.lo")?;
    Ok(encode_instruction(
        0x54000003 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

/// `b.hi` (cond `0b1000`) — branch if unsigned higher.
/// `B.PL` (N clear). After an `FCMP` this is the exact negation of the
/// float `<` guard: false when a < b (N set), true for a >= b AND for
/// unordered (NaN clears N) -- the IEEE skip condition for a `<` guard.
pub(in crate::aarch64) fn encode_conditional_branch_plus(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.pl")?;
    Ok(encode_instruction(
        0x54000005 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

/// `B.VC` (V clear) -- taken when the last flag-setting op did NOT overflow
/// signed arithmetic.
pub(in crate::aarch64) fn encode_conditional_branch_no_overflow(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.vc")?;
    Ok(encode_instruction(
        0x54000007 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_conditional_branch_higher(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.hi")?;
    Ok(encode_instruction(
        0x54000008 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

/// `b.ls` (cond `0b1001`) — branch if unsigned lower-or-same.
pub(in crate::aarch64) fn encode_conditional_branch_lower_or_same(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.ls")?;
    Ok(encode_instruction(
        0x54000009 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(in crate::aarch64) fn encode_cbz_x(
    register: u8,
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "cbz")?;
    Ok(encode_instruction(
        0xB4000000 | ((instruction_distance as u32 & 0x7ffff) << 5) | u32::from(register),
    ))
}

pub(in crate::aarch64) fn encode_cbnz_x(
    register: u8,
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "cbnz")?;
    Ok(encode_instruction(
        0xB5000000 | ((instruction_distance as u32 & 0x7ffff) << 5) | u32::from(register),
    ))
}

pub(in crate::aarch64) fn encode_unconditional_branch(
    byte_distance: isize,
) -> Result<[u8; 4], Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 26, "b")?;
    Ok(encode_instruction(
        0x14000000 | (instruction_distance as u32 & 0x03ff_ffff),
    ))
}

fn checked_instruction_distance(
    byte_distance: isize,
    immediate_bits: u8,
    instruction_name: &str,
) -> Result<isize, Diagnostic> {
    if byte_distance % 4 != 0 {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is not instruction aligned: {byte_distance} byte(s)"
        )));
    }

    let instruction_distance = byte_distance / 4;
    let min = -(1isize << (immediate_bits - 1));
    let max = (1isize << (immediate_bits - 1)) - 1;
    if instruction_distance < min || instruction_distance > max {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} target is out of range: {instruction_distance} instruction(s)"
        )));
    }

    Ok(instruction_distance)
}
