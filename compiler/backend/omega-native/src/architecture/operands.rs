use super::aarch64;
use crate::instructions::InstructionOperand;
use crate::target::Architecture;

pub fn operand_width(architecture: Architecture, operand: &InstructionOperand) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::operand_width(operand),
        Architecture::X86_64 => x86_64_operand_width(operand),
    }
}

fn x86_64_operand_width(_operand: &InstructionOperand) -> usize {
    8
}
