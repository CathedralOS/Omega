use omega_core::diagnostics::Diagnostic;

use super::instruction::encode_instruction;

pub(in crate::aarch64) fn encode_load_w_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<[u8; 4], Diagnostic> {
    match byte_size {
        1 => encode_load_byte_w_from_x(destination_register, base_register, byte_offset),
        2 => {
            // LDRH Wt, [Xn, #offset] — zero-extending halfword load.
            if !byte_offset.is_multiple_of(2) || byte_offset / 2 > 4095 {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load u16 at offset `{byte_offset}` yet"
                )));
            }
            Ok(encode_instruction(
                0x79400000
                    | (((byte_offset / 2) as u32) << 10)
                    | (u32::from(base_register) << 5)
                    | u32::from(destination_register),
            ))
        }
        4 => {
            if !byte_offset.is_multiple_of(4) || byte_offset / 4 > 4095 {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot load u32 guard at offset `{byte_offset}` yet"
                )));
            }
            Ok(encode_instruction(
                0xB9400000
                    | (((byte_offset / 4) as u32) << 10)
                    | (u32::from(base_register) << 5)
                    | u32::from(destination_register),
            ))
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot load {byte_size}-byte guard operands yet"
        ))),
    }
}

pub(in crate::aarch64) fn encode_load_byte_w17_from_x16(
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    encode_load_byte_w_from_x(17, 16, byte_offset)
}

pub(in crate::aarch64) fn encode_load_byte_w_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if byte_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot load byte at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0x39400000
            | ((byte_offset as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

pub(in crate::aarch64) fn encode_store_w17_to_x16(
    byte_offset: usize,
    byte_size: usize,
) -> Result<[u8; 4], Diagnostic> {
    encode_store_w_to_x(17, 16, byte_offset, byte_size)
}

pub(in crate::aarch64) fn encode_store_w_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<[u8; 4], Diagnostic> {
    match byte_size {
        1 => encode_store_byte_w_to_x(source_register, base_register, byte_offset),
        2 => {
            // STRH Wt, [Xn, #offset] — halfword store.
            if !byte_offset.is_multiple_of(2) || byte_offset / 2 > 4095 {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot store u16 at offset `{byte_offset}` yet"
                )));
            }
            Ok(encode_instruction(
                0x79000000
                    | (((byte_offset / 2) as u32) << 10)
                    | (u32::from(base_register) << 5)
                    | u32::from(source_register),
            ))
        }
        4 => {
            if !byte_offset.is_multiple_of(4) || byte_offset / 4 > 4095 {
                return Err(Diagnostic::error(format!(
                    "AArch64 MVP encoder cannot store u32 at offset `{byte_offset}` yet"
                )));
            }
            Ok(encode_instruction(
                0xB9000000
                    | (((byte_offset / 4) as u32) << 10)
                    | (u32::from(base_register) << 5)
                    | u32::from(source_register),
            ))
        }
        _ => Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
        ))),
    }
}

pub(in crate::aarch64) fn encode_store_x17_to_x16(
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    encode_store_x_to_x(17, 16, byte_offset)
}

pub(in crate::aarch64) fn encode_store_x_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store u64 at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0xF9000000
            | (((byte_offset / 8) as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

pub(in crate::aarch64) fn encode_load_x_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095 {
        return Ok(encode_instruction(
            0xF9400000
                | (((byte_offset / 8) as u32) << 10)
                | (u32::from(base_register) << 5)
                | u32::from(destination_register),
        ));
    }
    // Offsets that are not a multiple of 8 cannot use the scaled-offset LDR;
    // fall back to LDUR (unscaled signed 9-bit offset), still one instruction.
    if byte_offset <= 255 {
        return Ok(encode_instruction(
            0xF8400000
                | ((byte_offset as u32) << 12)
                | (u32::from(base_register) << 5)
                | u32::from(destination_register),
        ));
    }
    Err(Diagnostic::error(format!(
        "AArch64 MVP encoder cannot load u64 from x{base_register} at offset `{byte_offset}` yet"
    )))
}

pub(in crate::aarch64) fn encode_store_byte_w17_to_x16(
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    encode_store_byte_w_to_x(17, 16, byte_offset)
}

pub(in crate::aarch64) fn encode_store_byte_w_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if byte_offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot store byte at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0x39000000
            | ((byte_offset as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

pub(in crate::aarch64) fn encode_load_byte_w_post_increment(
    destination_register: u8,
    base_register: u8,
    byte_increment: i16,
) -> Result<[u8; 4], Diagnostic> {
    let immediate = signed_memory_immediate_9(byte_increment, "post-increment byte load")?;
    Ok(encode_instruction(
        0x38400400
            | (immediate << 12)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

pub(in crate::aarch64) fn encode_store_byte_w_post_increment(
    source_register: u8,
    base_register: u8,
    byte_increment: i16,
) -> Result<[u8; 4], Diagnostic> {
    let immediate = signed_memory_immediate_9(byte_increment, "post-increment byte store")?;
    Ok(encode_instruction(
        0x38000400
            | (immediate << 12)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

fn signed_memory_immediate_9(value: i16, instruction_name: &str) -> Result<u32, Diagnostic> {
    if !(-256..=255).contains(&value) {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} immediate is out of range: {value}"
        )));
    }
    Ok((i32::from(value) as u32) & 0x1ff)
}
