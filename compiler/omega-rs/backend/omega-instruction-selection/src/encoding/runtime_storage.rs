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
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_compare_bytes(
            left_offset,
            right_offset,
            byte_size,
            failure_branch_distance,
            operator,
            is_float,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_compare_bytes(
            left_offset,
            right_offset,
            byte_size,
            failure_branch_distance,
            operator,
            is_float,
        ),
    }
}

pub fn encode_runtime_storage_value_compare_bytes(
    architecture: Architecture,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_value_compare_bytes(
            byte_offset,
            byte_size,
            expected_value,
            failure_branch_distance,
            operator,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_value_compare_bytes(
            byte_offset,
            byte_size,
            expected_value,
            failure_branch_distance,
            operator,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_pointee_integer_write(
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            value,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: omega_core::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_binary_write(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_binary_write(
            runtime_value_operands,
            target_offset,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_convert(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_storage_convert(
            runtime_value_operands,
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_storage_convert(
            runtime_value_operands,
            target_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
        ),
    }
}

pub fn encode_atomic_fetch_add(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    delta: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_atomic_fetch_add(runtime_value_operands, target_offset, byte_size, delta)
        }
        Architecture::X86_64 => {
            x86_64::encode_atomic_fetch_add(runtime_value_operands, target_offset, byte_size, delta)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_atomic_compare_exchange(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_compare_exchange(
            runtime_value_operands,
            target_offset,
            byte_size,
            expected,
            new_value,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_compare_exchange(
            runtime_value_operands,
            target_offset,
            byte_size,
            expected,
            new_value,
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
        Architecture::X86_64 => x86_64::encode_runtime_pointee_binary_write(
            runtime_value_operands,
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_frame_indexed_integer_write(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_frame_base_indexed_integer_write(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
    }
}

pub fn encode_runtime_frame_base_indexed_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_base_indexed_binary_write(
            runtime_value_operands,
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_frame_base_indexed_binary_write(
            runtime_value_operands,
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_machine_indexed_binary_write(
            runtime_value_operands,
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_indexed_binary_write(
            runtime_value_operands,
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

pub fn encode_runtime_machine_indexed_integer_write(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_machine_indexed_integer_write(
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_indexed_integer_write(
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
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

pub fn encode_runtime_machine_bounded_buffer_write(
    architecture: Architecture,
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        // aarch64 owned-carrier write is not yet implemented; x86_64 (the native
        // run target) is. A carrier canary is therefore x86_64-run-only for now.
        Architecture::Aarch64 => Err(Diagnostic::error(
            "aarch64 encoder does not yet support the owned `[u8; N]` bounded byte carrier write"
                .to_string(),
        )),
        Architecture::X86_64 => {
            x86_64::encode_runtime_machine_bounded_buffer_write(byte_offset, literal)
        }
    }
}

pub fn encode_runtime_machine_bounded_buffer_source_append(
    architecture: Architecture,
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "aarch64 encoder does not yet support the owned `[u8; N]` bounded byte carrier append"
                .to_string(),
        )),
        Architecture::X86_64 => x86_64::encode_runtime_machine_bounded_buffer_source_append(
            target_byte_offset,
            source_byte_offset,
            source_in_frame,
        ),
    }
}

pub fn encode_runtime_machine_bounded_buffer_literal_append(
    architecture: Architecture,
    target_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "aarch64 encoder does not yet support the owned `[u8; N]` bounded byte carrier append"
                .to_string(),
        )),
        Architecture::X86_64 => {
            x86_64::encode_runtime_machine_bounded_buffer_literal_append(target_byte_offset, literal)
        }
    }
}

pub fn encode_runtime_frame_string_write(
    architecture: Architecture,
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_frame_string_write(byte_offset, byte_length)
        }
        Architecture::X86_64 => x86_64::encode_runtime_frame_string_write(byte_offset, byte_length),
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
        Architecture::X86_64 => x86_64::encode_runtime_pointee_string_write(
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
        ),
    }
}

pub fn encode_runtime_pointee_bounded_buffer_write(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "aarch64 encoder does not yet support the owned `[u8; N]` bounded byte carrier pointee write"
                .to_string(),
        )),
        Architecture::X86_64 => x86_64::encode_runtime_pointee_bounded_buffer_write(
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_frame_indexed_string_write(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_machine_indexed_string_write(
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_pointee_address_to_runtime_frame_write(
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
        ),
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

pub fn encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
    architecture: Architecture,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
    }
}

pub fn encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
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
        Architecture::X86_64 => x86_64::encode_runtime_storage_copy_from_runtime_frame_indexed(
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
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
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
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

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
    architecture: Architecture,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
                descriptor_offset,
                element_index,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
                descriptor_offset,
                element_index,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
                descriptor_offset,
                index_offset,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
                descriptor_offset,
                index_offset,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            )
        }
    }
}

pub fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            // aarch64 hardcodes a frame-resident index (pre-existing); the
            // index_region is consumed only by the x86_64 encoder for now.
            aarch64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                base_byte_offset,
                index_offset,
                index_region,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
    }
}

pub fn encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
    architecture: Architecture,
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            if source_region != omega_target_operations::RuntimeStorageRegion::Machine {
                return Err(Diagnostic::error(
                    "aarch64 cannot write a machine indexed element from a frame-resident                      source yet; use a machine field temp",
                ));
            }
            aarch64::encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
                source_offset,
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
                source_region,
                source_offset,
                base_byte_offset,
                index_offset,
                index_region,
                element_byte_size,
                field_byte_offset,
                byte_count,
            )
        }
    }
}

pub fn encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
    architecture: Architecture,
    source_base_byte_offset: usize,
    source_index_offset: usize,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_index_offset: usize,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => Err(Diagnostic::error(
            "aarch64 cannot encode a dual runtime-indexed copy (`arr[i] = arr[j]`) yet;              use a field temp",
        )),
        Architecture::X86_64 => x86_64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
            source_base_byte_offset,
            source_index_offset,
            source_index_region,
            source_element_byte_size,
            source_field_byte_offset,
            target_base_byte_offset,
            target_index_offset,
            target_index_region,
            target_element_byte_size,
            target_field_byte_offset,
            byte_count,
        ),
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
        Architecture::X86_64 => x86_64::encode_runtime_storage_copy_to_runtime_pointee(
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
        ),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime storage encoding is not implemented",
    ))
}
