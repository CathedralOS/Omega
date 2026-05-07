pub mod aarch64;

use crate::diagnostics::Diagnostic;
use crate::native::instructions::{InstructionOperand, InstructionOperandKind};
use crate::native::target::Architecture;

pub fn host_call_sequence_width(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::host_call_sequence_width(operands),
        Architecture::X86_64 => operands.len() * 8 + 5,
    }
}

pub fn return_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::return_width(),
        Architecture::X86_64 => 1,
    }
}

pub fn encode_host_call_sequence(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_host_call_sequence(operands),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_return(architecture: Architecture) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok(aarch64::encode_return()),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn operand_width(architecture: Architecture, operand: &InstructionOperand) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::operand_width(operand),
        Architecture::X86_64 => x86_64_operand_width(operand),
    }
}

fn x86_64_operand_width(_operand: &InstructionOperand) -> usize {
    8
}

pub fn operand_is_data_address(operand: &InstructionOperand) -> bool {
    matches!(operand.kind, InstructionOperandKind::DataAddress { .. })
}
