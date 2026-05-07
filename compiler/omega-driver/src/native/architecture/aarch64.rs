use crate::native::instructions::{InstructionOperand, InstructionOperandKind};

pub fn host_call_sequence_width(operands: &[InstructionOperand]) -> usize {
    operands.iter().map(operand_width).sum::<usize>() + 4
}

pub fn return_width() -> usize {
    4
}

pub fn operand_width(operand: &InstructionOperand) -> usize {
    match operand.kind {
        InstructionOperandKind::DataAddress { .. } => 8,
        InstructionOperandKind::ImmediateInteger(_) | InstructionOperandKind::ByteLength(_) => 4,
    }
}

pub fn encode_host_call_sequence(operands: &[InstructionOperand]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut next_register = 0u8;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::ImmediateInteger(value) => {
                bytes.extend(encode_movz(next_register, *value as u16));
                next_register += 1;
            }
            InstructionOperandKind::DataAddress { .. } => {
                bytes.extend(encode_adrp_placeholder(next_register));
                bytes.extend(encode_add_page_offset_placeholder(next_register));
                next_register += 1;
            }
            InstructionOperandKind::ByteLength(value) => {
                bytes.extend(encode_movz(next_register, *value as u16));
                next_register += 1;
            }
        }
    }

    bytes.extend(encode_branch_link_placeholder());
    bytes
}

pub fn encode_return() -> Vec<u8> {
    encode_instruction(0xD65F03C0)
}

fn encode_movz(register: u8, immediate: u16) -> Vec<u8> {
    encode_instruction(0xD2800000 | (u32::from(immediate) << 5) | u32::from(register))
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

fn encode_instruction(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}
