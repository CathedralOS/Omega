use omega_isa_aarch64::{Aarch64CallOperand, aarch64};
use omega_target::Architecture;
use omega_target_operations::InstructionOperandLike;

pub fn operand_width(architecture: Architecture, operand: &impl InstructionOperandLike) -> usize {
    match architecture {
        Architecture::Aarch64 => aarch64::operand_width(&aarch64_call_operand(operand)),
        Architecture::X86_64 => x86_64_operand_width(operand),
    }
}

pub(crate) fn aarch64_call_operand(operand: &impl InstructionOperandLike) -> Aarch64CallOperand {
    if operand.data_address().is_some() {
        Aarch64CallOperand::DataAddress
    } else if let Some((_, byte_offset)) = operand.runtime_string_pointer() {
        Aarch64CallOperand::RuntimeStringPointer { byte_offset }
    } else if let Some((_, byte_offset)) = operand.runtime_string_length() {
        Aarch64CallOperand::RuntimeStringLength { byte_offset }
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_pointer() {
        Aarch64CallOperand::RuntimePointeeStringPointer { byte_offset }
    } else if let Some((_, byte_offset)) = operand.runtime_pointee_string_length() {
        Aarch64CallOperand::RuntimePointeeStringLength { byte_offset }
    } else if let Some((_, byte_offset, _)) = operand.runtime_scalar_integer() {
        Aarch64CallOperand::RuntimeScalarInteger { byte_offset }
    } else if let Some(value) = operand.immediate_integer() {
        Aarch64CallOperand::ImmediateInteger(value)
    } else if let Some(value) = operand.byte_length() {
        Aarch64CallOperand::ByteLength(value)
    } else {
        unreachable!("instruction operand kind should map to an AArch64 call operand")
    }
}

fn x86_64_operand_width(_operand: &impl InstructionOperandLike) -> usize {
    8
}
