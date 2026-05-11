use omega_isa_aarch64::{Aarch64CallOperand, aarch64};
use omega_target::Architecture;
use omega_target_operations::{InstructionOperand, InstructionOperandKind};

pub fn operand_width(architecture: Architecture, operand: &InstructionOperand) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::operand_width(&aarch64_call_operand(operand)),
        Architecture::X86_64 => x86_64_operand_width(operand),
    }
}

pub fn aarch64_call_operands(operands: &[InstructionOperand]) -> Vec<Aarch64CallOperand> {
    operands.iter().map(aarch64_call_operand).collect()
}

fn aarch64_call_operand(operand: &InstructionOperand) -> Aarch64CallOperand {
    match &operand.kind {
        InstructionOperandKind::DataAddress { .. } => Aarch64CallOperand::DataAddress,
        InstructionOperandKind::RuntimeMachineStringPointer { byte_offset } => {
            Aarch64CallOperand::RuntimeMachineStringPointer {
                byte_offset: *byte_offset,
            }
        }
        InstructionOperandKind::RuntimeMachineStringLength { byte_offset } => {
            Aarch64CallOperand::RuntimeMachineStringLength {
                byte_offset: *byte_offset,
            }
        }
        InstructionOperandKind::ImmediateInteger(value) => {
            Aarch64CallOperand::ImmediateInteger(*value)
        }
        InstructionOperandKind::ByteLength(value) => Aarch64CallOperand::ByteLength(*value),
    }
}

fn x86_64_operand_width(_operand: &InstructionOperand) -> usize {
    8
}
