use omega_core::diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_add_x_immediate(
    destination_register: u8,
    source_register: u8,
    value: usize,
) -> Result<Vec<u8>, Diagnostic> {
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

pub(in crate::aarch64) fn encode_add_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Vec<u8> {
    encode_instruction(
        0x8B000000
            | (u32::from(right_register) << 16)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_sub_x_register(
    destination_register: u8,
    left_register: u8,
    right_register: u8,
) -> Vec<u8> {
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
) -> Vec<u8> {
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
) -> Vec<u8> {
    encode_instruction(
        0x9AC00800
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
) -> Vec<u8> {
    encode_instruction(
        0x9B008000
            | (u32::from(right_register) << 16)
            | (u32::from(minuend_register) << 10)
            | (u32::from(left_register) << 5)
            | u32::from(destination_register),
    )
}

pub(in crate::aarch64) fn encode_subs_x_immediate(
    destination_register: u8,
    source_register: u8,
    value: usize,
) -> Result<Vec<u8>, Diagnostic> {
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
