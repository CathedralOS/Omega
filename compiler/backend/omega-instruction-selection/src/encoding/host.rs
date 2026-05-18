use crate::aarch64_call_operands;
use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_target::Architecture;
use omega_target_operations::InstructionOperand;

pub fn encode_host_call_sequence(
    architecture: Architecture,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_host_call_sequence(&aarch64_call_operands(operands))
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
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
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_return(architecture: Architecture) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok(aarch64::encode_return()),
        Architecture::X86_64 => Ok(vec![0xC3]),
    }
}

pub fn encode_return_bytes(architecture: Architecture) -> Result<([u8; 4], usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Ok((aarch64::encode_return_bytes(), 4)),
        Architecture::X86_64 => Ok(([0xC3, 0, 0, 0], 1)),
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 host instruction encoding is not implemented",
    ))
}
