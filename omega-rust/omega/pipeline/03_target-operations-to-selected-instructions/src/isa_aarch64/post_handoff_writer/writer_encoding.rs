use diagnostics::Diagnostic;

fn instruction(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}

fn halfword(value: u64, shift: u8) -> u16 {
    ((value >> (u64::from(shift) * 16)) & 0xffff) as u16
}

fn encode_movz(register: u8, immediate: u16) -> [u8; 4] {
    instruction(0xD280_0000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movk(register: u8, immediate: u16, shift: u8) -> [u8; 4] {
    instruction(
        0xF280_0000 | (u32::from(shift) << 21) | (u32::from(immediate) << 5) | u32::from(register),
    )
}

pub(super) fn append_unsigned_immediate_padded(bytes: &mut Vec<u8>, register: u8, value: u64) {
    bytes.extend(encode_movz(register, halfword(value, 0)));
    for shift in 1..4 {
        bytes.extend(encode_movk(register, halfword(value, shift), shift));
    }
}

pub(super) fn encode_move_x_register(destination: u8, source: u8) -> [u8; 4] {
    instruction(0xAA00_03E0 | (u32::from(source) << 16) | u32::from(destination))
}

pub(super) fn encode_and_x_register(destination: u8, left: u8, right: u8) -> [u8; 4] {
    instruction(
        0x8A00_0000 | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination),
    )
}

pub(super) fn encode_orr_x_register(destination: u8, left: u8, right: u8) -> [u8; 4] {
    instruction(
        0xAA00_0000 | (u32::from(right) << 16) | (u32::from(left) << 5) | u32::from(destination),
    )
}

pub(super) fn encode_lsr_x_immediate(destination: u8, source: u8, shift: u8) -> [u8; 4] {
    instruction(
        0xD340_FC00
            | (u32::from(shift & 0x3f) << 16)
            | (u32::from(source) << 5)
            | u32::from(destination),
    )
}

pub(super) fn encode_lsl_x_immediate(destination: u8, source: u8, shift: u8) -> [u8; 4] {
    debug_assert!((1..64).contains(&shift));
    let immr = 64_u32 - u32::from(shift);
    let imms = 63_u32 - u32::from(shift);
    instruction(
        0xD340_0000
            | (immr << 16)
            | (imms << 10)
            | (u32::from(source) << 5)
            | u32::from(destination),
    )
}

fn encode_load_byte_w(destination: u8, base: u8, offset: usize) -> Result<[u8; 4], Diagnostic> {
    if offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 writer cannot load a byte at offset `{offset}`"
        )));
    }
    Ok(instruction(
        0x3940_0000 | ((offset as u32) << 10) | (u32::from(base) << 5) | u32::from(destination),
    ))
}

pub(super) fn encode_load_w_from_x(
    destination: u8,
    base: u8,
    offset: usize,
    byte_size: usize,
) -> Result<[u8; 4], Diagnostic> {
    match byte_size {
        1 => encode_load_byte_w(destination, base, offset),
        2 if offset.is_multiple_of(2) && offset / 2 <= 4095 => Ok(instruction(
            0x7940_0000
                | (((offset / 2) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(destination),
        )),
        4 if offset.is_multiple_of(4) && offset / 4 <= 4095 => Ok(instruction(
            0xB940_0000
                | (((offset / 4) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(destination),
        )),
        _ => Err(Diagnostic::error(format!(
            "AArch64 writer cannot load a {byte_size}-byte value at offset `{offset}`"
        ))),
    }
}

pub(super) fn encode_load_x_from_x(
    destination: u8,
    base: u8,
    offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if offset.is_multiple_of(8) && offset / 8 <= 4095 {
        return Ok(instruction(
            0xF940_0000
                | (((offset / 8) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(destination),
        ));
    }
    if offset <= 255 {
        return Ok(instruction(
            0xF840_0000 | ((offset as u32) << 12) | (u32::from(base) << 5) | u32::from(destination),
        ));
    }
    Err(Diagnostic::error(format!(
        "AArch64 writer cannot load a u64 at offset `{offset}`"
    )))
}

fn encode_store_byte_w(source: u8, base: u8, offset: usize) -> Result<[u8; 4], Diagnostic> {
    if offset > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 writer cannot store a byte at offset `{offset}`"
        )));
    }
    Ok(instruction(
        0x3900_0000 | ((offset as u32) << 10) | (u32::from(base) << 5) | u32::from(source),
    ))
}

pub(super) fn encode_store_w_to_x(
    source: u8,
    base: u8,
    offset: usize,
    byte_size: usize,
) -> Result<[u8; 4], Diagnostic> {
    match byte_size {
        1 => encode_store_byte_w(source, base, offset),
        2 if offset.is_multiple_of(2) && offset / 2 <= 4095 => Ok(instruction(
            0x7900_0000
                | (((offset / 2) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(source),
        )),
        4 if offset.is_multiple_of(4) && offset / 4 <= 4095 => Ok(instruction(
            0xB900_0000
                | (((offset / 4) as u32) << 10)
                | (u32::from(base) << 5)
                | u32::from(source),
        )),
        _ => Err(Diagnostic::error(format!(
            "AArch64 writer cannot store a {byte_size}-byte value at offset `{offset}`"
        ))),
    }
}

pub(super) fn encode_store_x_to_x(
    source: u8,
    base: u8,
    offset: usize,
) -> Result<[u8; 4], Diagnostic> {
    if !offset.is_multiple_of(8) || offset / 8 > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 writer cannot store a u64 at offset `{offset}`"
        )));
    }
    Ok(instruction(
        0xF900_0000 | (((offset / 8) as u32) << 10) | (u32::from(base) << 5) | u32::from(source),
    ))
}
