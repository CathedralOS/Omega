use crate::architecture::aarch64;
use crate::architecture::aarch64_call_operands;
use crate::instructions::InstructionOperand;
use omega_core::diagnostics::Diagnostic;
use omega_target::Architecture;

pub fn encode_host_call_sequence(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_host_call_sequence(&aarch64_call_operands(operands))
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_syscall_sequence(
    architecture: Architecture,
    operands: &[InstructionOperand],
    syscall_number: u32,
    number_register: u8,
    supervisor_call: u16,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_syscall_sequence(
            &aarch64_call_operands(operands),
            syscall_number,
            number_register,
            supervisor_call,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_return(architecture: Architecture) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok(aarch64::encode_return()),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}
