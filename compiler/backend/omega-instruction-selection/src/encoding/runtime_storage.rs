use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_target::Architecture;
use omega_target_operations::{RuntimeValueOperand, StateGuardOperator};

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

pub fn encode_runtime_storage_binary_write(
    architecture: Architecture,
    target_offset: usize,
    byte_size: usize,
    left: &RuntimeValueOperand,
    operator: StateGuardOperator,
    right: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_binary_write(
                target_offset,
                byte_size,
                left,
                operator,
                right,
            )
        }
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_frame_indexed_integer_write(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_indexed_integer_write(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}

pub fn encode_runtime_frame_indexed_binary_write(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: &RuntimeValueOperand,
    operator: StateGuardOperator,
    right: &RuntimeValueOperand,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_indexed_binary_write(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
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

pub fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    architecture: Architecture,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed(
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
        ),
        Architecture::X86_64 => Ok(Vec::new()),
    }
}
