use crate::architecture::aarch64;
use omega_core::diagnostics::Diagnostic;
use omega_target::Architecture;

pub fn encode_runtime_storage_compare(
    architecture: Architecture,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_compare(
            left_offset,
            right_offset,
            byte_size,
            failure_branch_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_storage_value_compare(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_value_compare(
            byte_offset,
            byte_size,
            expected_value,
            failure_branch_distance,
            branch_when_equal,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_machine_integer_write(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_machine_integer_write(byte_offset, byte_size, value)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_machine_string_write(
    architecture: Architecture,
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_machine_string_write(byte_offset, byte_length)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_storage_copy(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy(source_offset, target_offset, byte_count)
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}
