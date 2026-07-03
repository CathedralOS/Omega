use crate::aarch64_call_operand;
use omega_calling_conventions::HostOperationKey;
use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::InstructionOperandLike;

pub fn encode_host_call_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operation_key: HostOperationKey,
    operands: &[T],
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_host_call_sequence_from_operands(
            operands.iter().map(aarch64_call_operand),
        ),
        Architecture::X86_64 => x86_64::encode_host_call_sequence(operation_key, operands),
    }
}

pub fn encode_syscall_sequence<T: InstructionOperandLike>(
    architecture: Architecture,
    operands: &[T],
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
        // x86_64 Linux: number_register/supervisor_call are aarch64-shaped (X8/SVC);
        // the System V syscall ABI fixes RAX + `syscall` (0F 05), so they are unused.
        Architecture::X86_64 => {
            let _ = (number_register, supervisor_call);
            x86_64::encode_syscall_sequence(operands, syscall_number)
        }
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

pub fn encode_runtime_storage_copy_to_return_register_bytes(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_to_return_register_bytes(byte_offset, byte_size)
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_to_return_register_bytes(byte_offset, byte_size)
        }
    }
}

/// The entry prologue's inbound argument unmarshal (`mov [frame+off], rcx/rdx/
/// r8/r9`). x86_64 only; an aarch64 entry with parameters is a clean error
/// until its stub (x0-x3) exists.
pub fn encode_entry_argument_register_write_bytes(
    architecture: Architecture,
    argument_index: u8,
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "AArch64 MVP encoder cannot unmarshal entry arguments yet (the entry \
             prologue register store is x86_64-only)",
        )),
        Architecture::X86_64 => {
            x86_64::encode_entry_argument_register_write_bytes(argument_index, byte_offset)
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
