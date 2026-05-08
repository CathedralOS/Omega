use omega_core::diagnostics::Diagnostic;

pub(super) fn encode_movz(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

pub(super) fn encode_movz_w(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0x52800000 | (u32::from(immediate) << 5) | u32::from(register))
}

pub(super) fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> Vec<u8> {
    encode_instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

pub(super) fn encode_move_x_register(destination_register: u8, source_register: u8) -> Vec<u8> {
    encode_instruction(
        0xAA0003E0 | (u32::from(source_register) << 16) | u32::from(destination_register),
    )
}

pub(super) fn encode_adrp_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x90000000 | u32::from(register))
}

pub(super) fn encode_add_page_offset_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x91000000 | (u32::from(register) << 5) | u32::from(register))
}

pub(super) fn encode_branch_link_placeholder() -> Vec<u8> {
    encode_instruction(0x94000000)
}

pub(super) fn encode_svc(immediate: u16) -> Vec<u8> {
    encode_instruction(0xD4000001 | (u32::from(immediate) << 5))
}

pub(super) fn encode_compare_w19_immediate(value: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_compare_w_immediate(19, value)
}

pub(super) fn encode_compare_w17_immediate(value: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_compare_w_immediate(17, value)
}

pub(super) fn encode_compare_w_immediate(register: u8, value: u32) -> Result<Vec<u8>, Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare guard value `{value}` yet"
        )));
    }

    Ok(encode_instruction(
        0x7100001F | (value << 10) | (u32::from(register) << 5),
    ))
}

pub(super) fn encode_compare_w_register(left_register: u8, right_register: u8) -> Vec<u8> {
    encode_instruction(
        0x6B00001F | (u32::from(right_register) << 16) | (u32::from(left_register) << 5),
    )
}

pub(super) fn encode_load_w17_from_x16(
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_load_w_from_x(17, 16, byte_offset, byte_size)
}

pub(super) fn encode_load_w_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match byte_size {
        1 => encode_load_byte_w_from_x(destination_register, base_register, byte_offset),
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

pub(super) fn encode_load_byte_w17_from_x16(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_load_byte_w_from_x(17, 16, byte_offset)
}

pub(super) fn encode_runtime_text_input_delimiter_check(
    byte_offset: usize,
    failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_load_byte_w17_from_x16(byte_offset)?;
    bytes.extend(encode_compare_w17_immediate(10)?);
    bytes.extend(encode_conditional_branch_equal(24)?);
    bytes.extend(encode_compare_w17_immediate(13)?);
    bytes.extend(encode_conditional_branch_equal(16)?);
    bytes.extend(encode_compare_w17_immediate(0)?);
    bytes.extend(encode_conditional_branch_equal(8)?);
    bytes.extend(encode_unconditional_branch(failure_branch_distance)?);
    Ok(bytes)
}

pub(super) fn encode_load_byte_w_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
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

pub(super) fn encode_store_w17_to_x16(
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_store_w_to_x(17, 16, byte_offset, byte_size)
}

pub(super) fn encode_store_w_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match byte_size {
        1 => encode_store_byte_w_to_x(source_register, base_register, byte_offset),
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

pub(super) fn encode_store_x17_to_x16(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_store_x_to_x(17, 16, byte_offset)
}

pub(super) fn encode_store_x_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
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

pub(super) fn encode_load_x_from_x(
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !byte_offset.is_multiple_of(8) || byte_offset / 8 > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot load u64 at offset `{byte_offset}` yet"
        )));
    }
    Ok(encode_instruction(
        0xF9400000
            | (((byte_offset / 8) as u32) << 10)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

pub(super) fn encode_store_byte_w17_to_x16(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_store_byte_w_to_x(17, 16, byte_offset)
}

pub(super) fn encode_store_byte_w_to_x(
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
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

pub(super) fn encode_load_byte_w_post_increment(
    destination_register: u8,
    base_register: u8,
    byte_increment: i16,
) -> Result<Vec<u8>, Diagnostic> {
    let immediate = signed_memory_immediate_9(byte_increment, "post-increment byte load")?;
    Ok(encode_instruction(
        0x38400400
            | (immediate << 12)
            | (u32::from(base_register) << 5)
            | u32::from(destination_register),
    ))
}

pub(super) fn encode_store_byte_w_post_increment(
    source_register: u8,
    base_register: u8,
    byte_increment: i16,
) -> Result<Vec<u8>, Diagnostic> {
    let immediate = signed_memory_immediate_9(byte_increment, "post-increment byte store")?;
    Ok(encode_instruction(
        0x38000400
            | (immediate << 12)
            | (u32::from(base_register) << 5)
            | u32::from(source_register),
    ))
}

pub(super) fn encode_add_x_immediate(
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

pub(super) fn encode_add_x_register(
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

pub(super) fn encode_subs_x_immediate(
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

pub(super) fn encode_conditional_branch_not_equal(
    byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.ne")?;
    Ok(encode_instruction(
        0x54000001 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(super) fn encode_conditional_branch_equal(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.eq")?;
    Ok(encode_instruction(
        0x54000000 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

pub(super) fn encode_cbz_x(register: u8, byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "cbz")?;
    Ok(encode_instruction(
        0xB4000000 | ((instruction_distance as u32 & 0x7ffff) << 5) | u32::from(register),
    ))
}

pub(super) fn encode_unconditional_branch(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 26, "b")?;
    Ok(encode_instruction(
        0x14000000 | (instruction_distance as u32 & 0x03ff_ffff),
    ))
}

pub(super) fn signed_memory_immediate_9(
    value: i16,
    instruction_name: &str,
) -> Result<u32, Diagnostic> {
    if !(-256..=255).contains(&value) {
        return Err(Diagnostic::error(format!(
            "AArch64 {instruction_name} immediate is out of range: {value}"
        )));
    }
    Ok((i32::from(value) as u32) & 0x1ff)
}

pub(super) fn checked_instruction_distance(
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

pub(super) fn encode_instruction(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}

pub(super) fn encode_immediate(register: u8, value: i64) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode negative immediate `{value}` yet"
        ))
    })?;

    Ok(encode_unsigned_immediate(register, value))
}

pub(super) fn encode_unsigned_immediate(register: u8, value: u64) -> Vec<u8> {
    let mut bytes = encode_movz(register, halfword(value, 0));

    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }

    bytes
}

pub(super) fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
