use omega_isa_aarch64::{Aarch64CallOperand, aarch64};
use omega_target::Architecture;
use omega_target_operations::{InstructionOperand, InstructionOperandKind};

pub fn operand_width(architecture: Architecture, operand: &InstructionOperand) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::operand_width(&aarch64_call_operand(operand)),
        Architecture::X86_64 => x86_64_operand_width(operand),
    }
}

pub(crate) fn aarch64_call_operand(operand: &InstructionOperand) -> Aarch64CallOperand {
    match &operand.kind {
        InstructionOperandKind::DataAddress { .. } => Aarch64CallOperand::DataAddress,
        InstructionOperandKind::RuntimeStringPointer { byte_offset, .. } => {
            Aarch64CallOperand::RuntimeStringPointer {
                byte_offset: *byte_offset,
            }
        }
        InstructionOperandKind::RuntimeStringLength { byte_offset, .. } => {
            Aarch64CallOperand::RuntimeStringLength {
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
