use psi_diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_compare_w17_immediate(value: u32) -> Result<[u8; 4], Diagnostic> {
    encode_compare_w_immediate(17, value)
}

pub(in crate::aarch64) fn encode_compare_w_immediate(
    register: u8,
    value: u32,
) -> Result<[u8; 4], Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare guard value `{value}` yet"
        )));
    }

    Ok(encode_instruction(
        0x7100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

/// `CMP Xn, #imm` (64-bit SUBS immediate discarding the result): the X-form
/// twin of `encode_compare_w_immediate`, for counts/values whose high 32
/// bits matter (a shift count register compared against the type width).
pub(in crate::aarch64) fn encode_compare_x_immediate(
    register: u8,
    value: u32,
) -> Result<[u8; 4], Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare value `{value}` yet"
        )));
    }

    Ok(encode_instruction(
        0xF100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

pub(in crate::aarch64) fn encode_compare_w_register(
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0x6B00001F | (u32::from(right_register) << 16) | (u32::from(left_register) << 5),
    )
}

pub(in crate::aarch64) fn encode_compare_x_register(
    left_register: u8,
    right_register: u8,
) -> [u8; 4] {
    encode_instruction(
        0xEB00001F | (u32::from(right_register) << 16) | (u32::from(left_register) << 5),
    )
}
