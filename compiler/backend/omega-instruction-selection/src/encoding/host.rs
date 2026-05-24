use crate::aarch64_call_operand;
use omega_calling_conventions::HostOperationKey;
use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::InstructionOperand;

pub fn encode_host_call_sequence(
    architecture: Architecture,
    operation_key: HostOperationKey,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_host_call_sequence_from_operands(
            operands.iter().map(aarch64_call_operand),
        ),
        Architecture::X86_64 => x86_64::encode_host_call_sequence(operation_key, operands),
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
        Architecture::Aarch64 => aarch64::encode_syscall_sequence_from_operands(
            operands.iter().map(aarch64_call_operand),
            syscall_number,
            number_register,
            supervisor_call,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_function_enter_bytes(
    architecture: Architecture,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_function_enter_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => Ok((Vec::new(), 0)),
    }
}

pub fn encode_return_bytes(architecture: Architecture) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes = aarch64::encode_return_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => {
            let bytes = x86_64::encode_return_bytes().to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
    }
}

pub fn encode_return_register_integer_write_bytes(
    architecture: Architecture,
    byte_size: usize,
    value: i64,
) -> Result<(Vec<u8>, usize), Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let bytes =
                aarch64::encode_return_register_integer_write_bytes(byte_size, value)?.to_vec();
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
        Architecture::X86_64 => {
            let bytes = x86_64::encode_return_register_integer_write_bytes(byte_size, value)?;
            let byte_count = bytes.len();
            Ok((bytes, byte_count))
        }
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 host instruction encoding is not implemented",
    ))
}
