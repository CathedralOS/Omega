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
    domain: omega_core::arithmetic::ArithmeticDomain,
    target_signed: bool,
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
            domain,
            target_signed,
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
            domain,
            target_signed,
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
        Architecture::Aarch64 => aarch64::encode_atomic_fetch_add(
            runtime_value_operands,
            target_offset,
            byte_size,
            delta,
        ),
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
pub fn encode_runtime_machine_double_indexed_binary_write(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_machine_double_indexed_binary_write(
            runtime_value_operands,
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_stride,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_double_indexed_binary_write(
            runtime_value_operands,
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_stride,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
    }
}

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
        Architecture::X86_64 => x86_64::encode_runtime_frame_indexed_binary_write(
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
        Architecture::Aarch64 => {
            aarch64::encode_runtime_machine_bounded_buffer_write(byte_offset, literal)
        }
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
        Architecture::Aarch64 => aarch64::encode_runtime_machine_bounded_buffer_source_append(
            target_byte_offset,
            source_byte_offset,
            source_in_frame,
        ),
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
        Architecture::Aarch64 => aarch64::encode_runtime_machine_bounded_buffer_literal_append(
            target_byte_offset,
            literal,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_bounded_buffer_literal_append(
            target_byte_offset,
            literal,
        ),
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
        Architecture::Aarch64 => aarch64::encode_runtime_pointee_bounded_buffer_write(
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
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

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_address_to_runtime_frame_write(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                index_region,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_frame_indexed_deref_address_to_runtime_frame_write(
                descriptor_offset,
                index_offset,
                index_region,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
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

/// The MACHINE-base element-address write (the wide-referee borrow-recast
/// let). x86_64 only.
pub fn encode_runtime_machine_indexed_address_to_runtime_frame_write(
    architecture: Architecture,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_machine_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_machine_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            )
        }
    }
}

/// The `CopyPlaces` encoder: x86_64 routes through the place materializer,
/// which picks the emission shape from the pair itself; aarch64 serves the
/// RECOGNIZED transitional shapes by decomposing to the retired per-variant
/// encoders (byte-identical to what the retired kinds emitted) and refuses
/// anything else until the aarch64 materializer rung lands (no runtime
/// oracle to verify new byte layouts there).
pub fn encode_copy_places(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => {
            x86_64::encode_copy_places(source, target, byte_count).map(|(bytes, _)| bytes)
        }
        Architecture::Aarch64 => match classify_copy_places_shape(source, target) {
            CopyPlacesShape::Direct {
                source_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy(source_offset, target_offset, byte_count),
            CopyPlacesShape::ToPointee {
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_pointee(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FromPointee {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            // The retired fixed-indexed-to-pointee encoder folds
            // index*size into the source displacement; passing index 0 /
            // size 1 with the already-folded field reuses it for ANY
            // deref-to-deref pair. Both pointer slots must be
            // frame-resident (the encoder reuses the frame base).
            CopyPlacesShape::PointeePair {
                source_pointer_byte_offset,
                source_field_byte_offset,
                target_pointer_byte_offset,
                target_field_byte_offset,
            } if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                aarch64::encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
                    source_pointer_byte_offset,
                    0,
                    1,
                    source_field_byte_offset,
                    target_pointer_byte_offset,
                    target_field_byte_offset,
                    byte_count,
                )
            }
            // The runtime-indexed decomposes: descriptor + index slots are
            // frame-resident by classification; the place regions must match
            // the retired encoders' frame assumptions or refuse loudly.
            CopyPlacesShape::FromIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                match target.region {
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                        aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed(
                            descriptor_offset,
                            index_offset,
                            element_byte_size,
                            field_byte_offset,
                            target_offset,
                            byte_count,
                        )
                    }
                    omega_target_operations::RuntimeStorageRegion::Machine => {
                        aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
                            descriptor_offset,
                            index_offset,
                            element_byte_size,
                            field_byte_offset,
                            target_offset,
                            byte_count,
                        )
                    }
                }
            }
            CopyPlacesShape::ToIndexed {
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed(
                    source_offset,
                    descriptor_offset,
                    index_offset,
                    element_byte_size,
                    field_byte_offset,
                    byte_count,
                )
            }
            CopyPlacesShape::IndexedToPointee {
                descriptor_offset,
                index_offset,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            } if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
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
            // The machine inline-array decomposes: the encoders take the
            // index region themselves (a frame-resident index reloads the
            // frame base mid-sequence).
            CopyPlacesShape::FromMachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                base_byte_offset,
                index_offset,
                index_region,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            CopyPlacesShape::ToMachineIndexed {
                source_offset,
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
                source_offset,
                base_byte_offset,
                index_offset,
                index_region,
                element_byte_size,
                field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FromFrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            CopyPlacesShape::PointeePair { .. }
            | CopyPlacesShape::FromIndexed { .. }
            | CopyPlacesShape::ToIndexed { .. }
            | CopyPlacesShape::IndexedToPointee { .. }
            | CopyPlacesShape::General => Err(Diagnostic::error(
                "CopyPlaces on aarch64 serves direct, single-pointee, pointee-pair, \
                 frame-rooted single-indexed, and inline-array place shapes only \
                 until the aarch64 place materializer lands; this shape refuses \
                 loudly",
            )),
        },
    }
}

/// The place-pair shapes the TRANSITIONAL aarch64 path recognizes. The
/// relocation walker and the encoder classify with the SAME function, so a
/// pair either decomposes consistently in both or refuses at layout time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyPlacesShape {
    /// Both paths are pure const offsets: the retired plain copy.
    Direct {
        source_offset: usize,
        target_offset: usize,
    },
    /// Direct source into a deref target (`*(base[ptr]) + field`): the
    /// retired to-pointee copy. The pointer slot lives in the target
    /// place's own region.
    ToPointee {
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    /// Deref source into a direct target: the retired from-pointee copy.
    FromPointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// Both sides deref (a fixed-indexed or pointee read landing through a
    /// pointer slot): the retired fixed-indexed-to-pointee copy.
    PointeePair {
        source_pointer_byte_offset: usize,
        source_field_byte_offset: usize,
        target_pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    /// Runtime-indexed source into a direct target: the retired
    /// from-frame-indexed copies (the descriptor and index slots are
    /// frame-resident in every producible instance).
    FromIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// Direct source into a runtime-indexed target: the retired
    /// to-frame-indexed element write.
    ToIndexed {
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    /// Runtime-indexed source landing through a pointer slot.
    IndexedToPointee {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    /// A MACHINE-resident inline array element read (no deref -- the array
    /// is machine statics, not a descriptor): the retired
    /// machine-indexed-to-storage copy. The index slot's region varies.
    FromMachineIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// The machine inline-array element WRITE.
    ToMachineIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    /// A FRAME-resident inline-array element read into a frame slot (the
    /// retired frame-base-indexed copy): all-frame, single index, no deref.
    FromFrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// Anything else (multi-index, multi-deref): x86_64-materializer only.
    General,
}

pub fn classify_copy_places_shape(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> CopyPlacesShape {
    // MACHINE inline-array shapes first (no deref -- the array lives in
    // machine statics): the index slot's region rides the ScaledIndex step.
    // A FRAME-rooted no-deref indexed place (the FrameBaseIndexed family)
    // stays General until its rung.
    if let Some(indexed) = direct_indexed_path(source) {
        if source.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some(target_offset) = target.const_offset()
        {
            return CopyPlacesShape::FromMachineIndexed {
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
                target_offset,
            };
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some(target_offset) = target.const_offset()
        {
            return CopyPlacesShape::FromFrameBaseIndexed {
                base_byte_offset: indexed.pointer_offset,
                index_offset: indexed.index_offset,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
                target_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    if let Some(indexed) = direct_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToMachineIndexed {
                source_offset,
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    // The indexed shapes first: an indexed path is NOT a single-deref path,
    // so these never shadow the pointee arms below. Frame-resident index
    // slots only (the retired encoders' assumption); anything else falls to
    // General.
    if let Some(indexed) = single_indexed_path(source) {
        if indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            if let Some(target_offset) = target.const_offset() {
                return CopyPlacesShape::FromIndexed {
                    descriptor_offset: indexed.pointer_offset,
                    index_offset: indexed.index_offset,
                    element_byte_size: indexed.element_byte_size,
                    field_byte_offset: indexed.field_offset,
                    target_offset,
                };
            }
            if let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
            {
                return CopyPlacesShape::IndexedToPointee {
                    descriptor_offset: indexed.pointer_offset,
                    index_offset: indexed.index_offset,
                    element_byte_size: indexed.element_byte_size,
                    source_field_byte_offset: indexed.field_offset,
                    pointer_byte_offset,
                    target_field_byte_offset,
                };
            }
        }
        return CopyPlacesShape::General;
    }
    if let Some(indexed) = single_indexed_path(target) {
        if indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToIndexed {
                source_offset,
                descriptor_offset: indexed.pointer_offset,
                index_offset: indexed.index_offset,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    match (
        source.const_offset(),
        target.const_offset(),
        single_deref_path(source),
        single_deref_path(target),
    ) {
        (Some(source_offset), Some(target_offset), _, _) => CopyPlacesShape::Direct {
            source_offset,
            target_offset,
        },
        (Some(source_offset), None, _, Some((pointer_byte_offset, field_byte_offset))) => {
            CopyPlacesShape::ToPointee {
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            }
        }
        (None, Some(target_offset), Some((pointer_byte_offset, field_byte_offset)), _) => {
            CopyPlacesShape::FromPointee {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            }
        }
        (
            None,
            None,
            Some((source_pointer_byte_offset, source_field_byte_offset)),
            Some((target_pointer_byte_offset, target_field_byte_offset)),
        ) => CopyPlacesShape::PointeePair {
            source_pointer_byte_offset,
            source_field_byte_offset,
            target_pointer_byte_offset,
            target_field_byte_offset,
        },
        _ => CopyPlacesShape::General,
    }
}

/// One runtime-indexed hop: `[ConstOffset(p)?, Deref, ScaledIndex, ConstOffset(f)?]`.
struct SingleIndexedPath {
    pointer_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    element_byte_size: usize,
    field_offset: usize,
}

/// A DIRECT indexed hop (no deref -- the inline-array shape):
/// `[ConstOffset(base)?, ScaledIndex, ConstOffset(field)?]`.
fn direct_indexed_path(place: &omega_target_operations::Place) -> Option<SingleIndexedPath> {
    let mut steps = place.steps().iter();
    let mut pointer_offset = 0usize;
    let (index_region, index_offset, element_byte_size) = loop {
        match steps.next() {
            Some(omega_target_operations::PlaceStep::ConstOffset(offset)) => {
                pointer_offset += offset
            }
            Some(omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                element_byte_size,
            }) => break (*index_region, *index_offset, *element_byte_size),
            _ => return None,
        }
    };
    let mut field_offset = 0usize;
    for step in steps {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) => field_offset += offset,
            _ => return None,
        }
    }
    Some(SingleIndexedPath {
        pointer_offset,
        index_region,
        index_offset,
        element_byte_size,
        field_offset,
    })
}

fn single_indexed_path(place: &omega_target_operations::Place) -> Option<SingleIndexedPath> {
    let mut steps = place.steps().iter();
    let mut pointer_offset = 0usize;
    loop {
        match steps.next() {
            Some(omega_target_operations::PlaceStep::ConstOffset(offset)) => {
                pointer_offset += offset
            }
            Some(omega_target_operations::PlaceStep::Deref) => break,
            _ => return None,
        }
    }
    let Some(omega_target_operations::PlaceStep::ScaledIndex {
        index_region,
        index_offset,
        element_byte_size,
    }) = steps.next()
    else {
        return None;
    };
    let mut field_offset = 0usize;
    for step in steps {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) => field_offset += offset,
            _ => return None,
        }
    }
    Some(SingleIndexedPath {
        pointer_offset,
        index_region: *index_region,
        index_offset: *index_offset,
        element_byte_size: *element_byte_size,
        field_offset,
    })
}

/// `[ConstOffset(p)?, Deref, ConstOffset(f)?]` -> `(p, f)`; anything else
/// (no deref, several derefs, an index) is `None`.
fn single_deref_path(place: &omega_target_operations::Place) -> Option<(usize, usize)> {
    let mut steps = place.steps().iter();
    let mut pointer_offset = 0usize;
    loop {
        match steps.next() {
            Some(omega_target_operations::PlaceStep::ConstOffset(offset)) => {
                pointer_offset += offset
            }
            Some(omega_target_operations::PlaceStep::Deref) => break,
            _ => return None,
        }
    }
    let mut field_offset = 0usize;
    for step in steps {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) => field_offset += offset,
            _ => return None,
        }
    }
    Some((pointer_offset, field_offset))
}

/// The x86_64 `CopyPlaces` encode WITH its relocation sites -- the
/// relocation walker's source of truth for where each base mov sits (the
/// SAME walk that emits the bytes; by relocation time layout has already
/// encoded this shape successfully, so a refusal here is unreachable).
pub fn x86_64_encode_copy_places_with_sites(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
    byte_count: usize,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_copy_places(source, target, byte_count)
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
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
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
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
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
            )
        }
    }
}

pub fn encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
    architecture: Architecture,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_stride,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_stride,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
    }
}

pub fn encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
    architecture: Architecture,
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
                source_region,
                source_offset,
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_stride,
                field_byte_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
                source_region,
                source_offset,
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_stride,
                field_byte_offset,
                byte_count,
            )
        }
    }
}

pub fn encode_runtime_machine_double_indexed_integer_write(
    architecture: Architecture,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_machine_double_indexed_integer_write(
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_stride,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_double_indexed_integer_write(
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_stride,
            field_byte_offset,
            byte_size,
            value,
        ),
    }
}

pub fn encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
    architecture: Architecture,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_offset,
                outer_stride,
                inner_index_offset,
                inner_stride,
                field_byte_offset,
                target_offset,
                byte_count,
            )
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_offset,
                outer_stride,
                inner_index_offset,
                inner_stride,
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
