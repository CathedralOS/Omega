use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};

pub fn encode_runtime_storage_compare_bytes(
    architecture: Architecture,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<[u8; 32], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_compare_bytes(
            left_offset,
            right_offset,
            byte_size,
            failure_branch_distance,
            operator,
        ),
        Architecture::X86_64 => unsupported_x86_64_runtime_storage_compare_encoding(),
    }
}

pub fn encode_runtime_storage_value_compare_bytes(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<[u8; 20], Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_value_compare_bytes(
            byte_offset,
            byte_size,
            expected_value,
            failure_branch_distance,
            operator,
        ),
        Architecture::X86_64 => unsupported_x86_64_runtime_storage_value_compare_encoding(),
    }
}

pub fn encode_runtime_value_compare(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    left: RuntimeValueOperandHandle,
    right: RuntimeValueOperandHandle,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_value_compare(
            runtime_value_operands,
            left,
            right,
            byte_size,
            failure_branch_distance,
            operator,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_value_compare(
            runtime_value_operands,
            left,
            right,
            byte_size,
            failure_branch_distance,
            operator,
        ),
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
        Architecture::X86_64 => {
            x86_64::encode_runtime_machine_integer_write(byte_offset, byte_size, value)
        }
    }
}

pub fn encode_runtime_pointee_integer_write(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_pointee_integer_write(
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_binary_write(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_binary_write(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

pub fn encode_runtime_pointee_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_pointee_binary_write(
            runtime_value_operands,
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
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
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_base_indexed_integer_write(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_machine_indexed_integer_write(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_machine_indexed_integer_write(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_frame_indexed_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_indexed_binary_write(
            runtime_value_operands,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
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
        Architecture::X86_64 => {
            x86_64::encode_runtime_machine_string_write(byte_offset, byte_length)
        }
    }
}

pub fn encode_runtime_pointee_string_write(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_pointee_string_write(
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_frame_indexed_string_write(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_indexed_string_write(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_machine_indexed_string_write(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_machine_indexed_string_write(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_address_to_runtime_frame_write(
    architecture: Architecture,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_address_to_runtime_frame_write(
            source_offset,
            target_offset,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_address_to_runtime_frame_write(
            source_offset,
            target_offset,
        ),
    }
}

pub fn encode_runtime_pointee_address_to_runtime_frame_write(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_pointee_address_to_runtime_frame_write(
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_frame_indexed_address_to_runtime_frame_write(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
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
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy(source_offset, target_offset, byte_count)
        }
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
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
    architecture: Architecture,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
    architecture: Architecture,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_storage_copy_to_runtime_pointee(
    architecture: Architecture,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_copy_to_runtime_pointee(
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
        ),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime storage encoding is not implemented",
    ))
}

fn unsupported_x86_64_runtime_storage_compare_encoding() -> Result<[u8; 32], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime storage encoding is not implemented",
    ))
}

fn unsupported_x86_64_runtime_storage_value_compare_encoding() -> Result<[u8; 20], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime storage encoding is not implemented",
    ))
}
