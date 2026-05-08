use omega_core::diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::architecture::aarch64) fn encode_compare_w19_immediate(
    value: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_compare_w_immediate(19, value)
}

pub(in crate::architecture::aarch64) fn encode_compare_w17_immediate(
    value: u32,
) -> Result<Vec<u8>, Diagnostic> {
    encode_compare_w_immediate(17, value)
}

pub(in crate::architecture::aarch64) fn encode_compare_w_immediate(
    register: u8,
    value: u32,
) -> Result<Vec<u8>, Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare guard value `{value}` yet"
        )));
    }

    Ok(encode_instruction(
        0x7100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

pub(in crate::architecture::aarch64) fn encode_compare_w_register(
    left_register: u8,
    right_register: u8,
) -> Vec<u8> {
    encode_instruction(
        0x6B00001F | (u32::from(right_register) << 16) | (u32::from(left_register) << 5),
    )
}
