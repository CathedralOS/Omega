use crate::diagnostics::Diagnostic;
use crate::native::instructions::{InstructionOperand, InstructionOperandKind};

pub fn host_call_sequence_width(operands: &[InstructionOperand]) -> usize {
    operands.iter().map(operand_width).sum::<usize>() + 4
}

pub fn return_width() -> usize {
    4
}

pub fn dispatch_loop_enter_width() -> usize {
    4
}

pub fn dispatch_case_enter_width() -> usize {
    8
}

pub fn dispatch_state_write_width() -> usize {
    4
}

pub fn dispatch_case_leave_width() -> usize {
    4
}

pub fn operand_width(operand: &InstructionOperand) -> usize {
    match &operand.kind {
        InstructionOperandKind::DataAddress { .. } => 8,
        InstructionOperandKind::ImmediateInteger(value) => immediate_width(*value),
        InstructionOperandKind::ByteLength(value) => unsigned_immediate_width(*value as u64),
    }
}

pub fn encode_host_call_sequence(operands: &[InstructionOperand]) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::new();
    let mut next_register = 0u8;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::ImmediateInteger(value) => {
                bytes.extend(encode_immediate(next_register, *value)?);
                next_register += 1;
            }
            InstructionOperandKind::DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            InstructionOperandKind::ByteLength(value) => {
                bytes.extend(encode_unsigned_immediate(next_register, *value as u64));
                next_register += 1;
            }
        }
    }

    bytes.extend(encode_branch_link_placeholder());
    Ok(bytes)
}

pub fn encode_return() -> Vec<u8> {
    encode_instruction(0xD65F03C0)
}

pub fn encode_dispatch_loop_enter(entry_dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    encode_dispatch_state_write(entry_dispatch_index)
}

pub fn encode_dispatch_case_enter(
    dispatch_index: u32,
    skip_byte_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_compare_w19_immediate(dispatch_index)?;
    bytes.extend(encode_conditional_branch_not_equal(skip_byte_distance)?);
    Ok(bytes)
}

pub fn encode_dispatch_state_write(dispatch_index: u32) -> Result<Vec<u8>, Diagnostic> {
    let immediate = u16::try_from(dispatch_index).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode dispatch index `{dispatch_index}` yet"
        ))
    })?;
    Ok(encode_movz_w(19, immediate))
}

pub fn encode_dispatch_case_leave(loop_byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    encode_unconditional_branch(loop_byte_distance)
}

fn encode_movz(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movz_w(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0x52800000 | (u32::from(immediate) << 5) | u32::from(register))
}

fn encode_movk(register: u8, immediate: u16, halfword_shift: u8) -> Vec<u8> {
    encode_instruction(
        0xF2800000
            | (u32::from(halfword_shift) << 21)
            | (u32::from(immediate) << 5)
            | u32::from(register),
    )
}

fn encode_adrp_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x90000000 | u32::from(register))
}

fn encode_add_page_offset_placeholder(register: u8) -> Vec<u8> {
    encode_instruction(0x91000000 | (u32::from(register) << 5) | u32::from(register))
}

fn encode_branch_link_placeholder() -> Vec<u8> {
    encode_instruction(0x94000000)
}

fn encode_compare_w19_immediate(value: u32) -> Result<Vec<u8>, Diagnostic> {
    if value > 4095 {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot compare dispatch index `{value}` yet"
        )));
    }

    Ok(encode_instruction(
        0x7100001F | (value << 10) | (u32::from(19u8) << 5),
    ))
}

fn encode_conditional_branch_not_equal(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
    let instruction_distance = checked_instruction_distance(byte_distance, 19, "b.ne")?;
    Ok(encode_instruction(
        0x54000001 | ((instruction_distance as u32 & 0x7ffff) << 5),
    ))
}

fn encode_unconditional_branch(byte_distance: isize) -> Result<Vec<u8>, Diagnostic> {
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

fn encode_instruction(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}

fn encode_immediate(register: u8, value: i64) -> Result<Vec<u8>, Diagnostic> {
    let value = u64::try_from(value).map_err(|_| {
        Diagnostic::error(format!(
            "AArch64 MVP encoder cannot encode negative immediate `{value}` yet"
        ))
    })?;

    Ok(encode_unsigned_immediate(register, value))
}

fn encode_unsigned_immediate(register: u8, value: u64) -> Vec<u8> {
    let mut bytes = encode_movz(register, halfword(value, 0));

    for halfword_shift in 1..4 {
        let immediate = halfword(value, halfword_shift);
        if immediate != 0 {
            bytes.extend(encode_movk(register, immediate, halfword_shift));
        }
    }

    bytes
}

fn immediate_width(value: i64) -> usize {
    match u64::try_from(value) {
        Ok(value) => unsigned_immediate_width(value),
        Err(_) => 4,
    }
}

fn unsigned_immediate_width(value: u64) -> usize {
    let high_nonzero_halfwords = (1..4)
        .filter(|halfword_shift| halfword(value, *halfword_shift) != 0)
        .count();

    4 + high_nonzero_halfwords * 4
}

fn halfword(value: u64, halfword_shift: u8) -> u16 {
    ((value >> (u64::from(halfword_shift) * 16)) & 0xffff) as u16
}
