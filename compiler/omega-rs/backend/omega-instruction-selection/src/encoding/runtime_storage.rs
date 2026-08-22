use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::{
    RuntimeBitFieldFragment, RuntimeValueOperandHandle, RuntimeValueOperandSource,
    StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

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

pub fn encode_runtime_storage_bit_field_write(
    architecture: Architecture,
    base_byte_offset: usize,
    fragments: &[RuntimeBitFieldFragment],
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_storage_bit_field_write(base_byte_offset, fragments, value)
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_storage_bit_field_write(base_byte_offset, fragments, value)
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
    domain: psi_numerics::arithmetic::ArithmeticDomain,
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
    target_signed: bool,
    trapping: bool,
    saturating: bool,
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
            target_signed,
            trapping,
            saturating,
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
            target_signed,
            trapping,
            saturating,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_write_place_convert(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &omega_target_operations::Place,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<Vec<u8>, Diagnostic> {
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) = classify_frame_base_indexed_convert_shape(target)
    {
        return aarch64::encode_runtime_frame_base_indexed_convert_write_with_index_region(
            runtime_value_operands,
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_double) = classify_frame_base_double_indexed_convert_shape(target)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_convert_write(
            runtime_value_operands,
            frame_double.base_byte_offset,
            frame_double.outer_index_offset,
            frame_double.outer_index_byte_size,
            frame_double.outer_stride,
            frame_double.inner_index_offset,
            frame_double.inner_index_byte_size,
            frame_double.inner_stride,
            frame_double.field_byte_offset,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        );
    }
    match architecture {
        Architecture::X86_64 => x86_64::encode_place_convert_write(
            runtime_value_operands,
            target,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        )
        .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => match classify_write_place_shape(target) {
            WritePlaceShape::Direct { byte_offset } => aarch64::encode_runtime_storage_convert(
                runtime_value_operands,
                byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => aarch64::encode_runtime_pointee_convert_write(
                runtime_value_operands,
                pointer_byte_offset,
                field_byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            WritePlaceShape::FrameIndexed {
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_convert_write(
                runtime_value_operands,
                descriptor_offset,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            WritePlaceShape::FrameIndexedByRegion {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_convert_write(
                runtime_value_operands,
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_convert_write(
                runtime_value_operands,
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_convert_write(
                runtime_value_operands,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_convert_write(
                runtime_value_operands,
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            ),
            _ => Err(Diagnostic::error(
                "WritePlaceConvert on aarch64 does not serve this unclassified place shape",
            )),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn write_place_convert_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &omega_target_operations::Place,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<usize, Diagnostic> {
    encode_write_place_convert(
        architecture,
        runtime_value_operands,
        target,
        target_byte_size,
        source,
        source_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    )
    .map(|bytes| bytes.len())
}

#[allow(clippy::too_many_arguments)]
pub fn x86_64_encode_write_place_convert_with_sites(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &omega_target_operations::Place,
    target_byte_size: usize,
    source: RuntimeValueOperandHandle,
    source_byte_size: usize,
    source_is_float: bool,
    target_is_float: bool,
    source_signed: bool,
    target_signed: bool,
    trapping: bool,
    saturating: bool,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_convert_write(
        runtime_value_operands,
        target,
        target_byte_size,
        source,
        source_byte_size,
        source_is_float,
        target_is_float,
        source_signed,
        target_signed,
        trapping,
        saturating,
    )
}

pub fn encode_atomic_load_to_storage(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::Load(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic load reached code generation without a load ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_load_to_storage(
            source_offset,
            byte_size,
            result_offset,
            ordering,
        ),
        Architecture::X86_64 => {
            x86_64::encode_atomic_load_to_storage(source_offset, byte_size, result_offset)
        }
    }
}

pub fn encode_atomic_store_from_operand(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::Store(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic store reached code generation without a store ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_store_from_operand(
            runtime_value_operands,
            target_offset,
            byte_size,
            value,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_store_from_operand(
            runtime_value_operands,
            target_offset,
            byte_size,
            value,
            ordering == psi_language_core::MemoryOrdering::GlobalOrder,
        ),
    }
}

pub fn encode_atomic_fetch_add(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic fetch_add reached code generation without an RMW ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_fetch_add(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_fetch_add(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta,
        ),
    }
}

pub fn encode_atomic_fetch_sub(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    delta: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic fetch_sub reached code generation without an RMW ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_fetch_sub(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_fetch_sub(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            delta,
        ),
    }
}

pub fn encode_atomic_fetch_xor(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic fetch_xor reached code generation without an RMW ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_fetch_xor(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_fetch_xor(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
        ),
    }
}

pub fn encode_atomic_fetch_or(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic fetch_or reached code generation without an RMW ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_fetch_or(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_fetch_or(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
        ),
    }
}

pub fn encode_atomic_fetch_and(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic fetch_and reached code generation without an RMW ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_fetch_and(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_fetch_and(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            value,
        ),
    }
}

pub fn encode_atomic_swap(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::Swap(ordering) = ordering else {
        return Err(Diagnostic::error(
            "atomic swap reached code generation without a swap ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_swap(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            new_value,
            ordering,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_swap(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            new_value,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_atomic_compare_exchange(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    expected: RuntimeValueOperandHandle,
    new_value: RuntimeValueOperandHandle,
    ordering: psi_language_core::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let psi_language_core::AtomicOrderingPlan::CompareExchange { success, .. } = ordering else {
        return Err(Diagnostic::error(
            "atomic compare_exchange reached code generation without a CAS ordering plan",
        ));
    };
    match architecture {
        Architecture::Aarch64 => aarch64::encode_atomic_compare_exchange(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
            expected,
            new_value,
            success,
        ),
        Architecture::X86_64 => x86_64::encode_atomic_compare_exchange(
            runtime_value_operands,
            target_offset,
            byte_size,
            result_offset,
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_indexed_integer_write(
            descriptor_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_frame_indexed_integer_write(
            descriptor_offset,
            index_offset,
            index_byte_size,
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_frame_base_indexed_integer_write(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_frame_base_indexed_integer_write(
            base_byte_offset,
            index_offset,
            index_byte_size,
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
    index_byte_size: usize,
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
            index_byte_size,
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
            index_byte_size,
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
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
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
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_index_byte_size,
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
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_index_byte_size,
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
    index_byte_size: usize,
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
            index_byte_size,
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
            index_byte_size,
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
    index_byte_size: usize,
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
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_indexed_integer_write(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
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
    index_byte_size: usize,
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
            index_byte_size,
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
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ),
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
    literal: &[u8],
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

/// The `CopyPlaces` encoder: x86_64 routes through the place materializer,
/// which picks the emission shape from the pair itself; aarch64 serves the
/// RECOGNIZED transitional shapes by decomposing to the retired per-variant
/// encoders (byte-identical to what the retired kinds emitted) and refuses
/// anything else until the aarch64 materializer rung lands (no runtime
/// oracle to verify new byte layouts there).
/// The single-place shapes the TRANSITIONAL aarch64 write path recognizes
/// (the CopyPlacesShape twin for one place). The walker and the encoder
/// classify with the SAME function, so a place either decomposes
/// consistently in both or refuses at layout time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePlaceShape {
    Direct {
        byte_offset: usize,
    },
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameIndexedByRegion {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    PointeeDoubleIndexed {
        descriptor_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    /// x86_64-materializer only.
    Unsupported,
}

pub fn classify_write_place_shape(target: &omega_target_operations::Place) -> WritePlaceShape {
    if let Some(byte_offset) = target.const_offset() {
        return WritePlaceShape::Direct { byte_offset };
    }
    if let Some(double) = direct_double_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine {
            return WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                field_byte_offset: double.field_offset,
            };
        }
        return WritePlaceShape::Unsupported;
    }
    if let Some(double) = pointee_double_indexed_path(target) {
        return WritePlaceShape::PointeeDoubleIndexed {
            descriptor_offset: double.descriptor_offset,
            outer_index_region: double.outer_region,
            outer_index_offset: double.outer_offset,
            outer_index_byte_size: double.outer_byte_size,
            outer_stride: double.outer_stride,
            inner_index_region: double.inner_region,
            inner_index_offset: double.inner_offset,
            inner_index_byte_size: double.inner_byte_size,
            inner_stride: double.inner_stride,
            field_byte_offset: double.field_offset,
        };
    }
    if let Some(indexed) = direct_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine {
            return WritePlaceShape::MachineIndexed {
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        {
            return WritePlaceShape::FrameBaseIndexed {
                base_byte_offset: indexed.pointer_offset,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        return WritePlaceShape::Unsupported;
    }
    if let Some(indexed) = single_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            if indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                return WritePlaceShape::FrameIndexed {
                    descriptor_offset: indexed.pointer_offset,
                    index_offset: indexed.index_offset,
                    index_byte_size: indexed.index_byte_size,
                    element_byte_size: indexed.element_byte_size,
                    field_byte_offset: indexed.field_offset,
                };
            }
            return WritePlaceShape::FrameIndexedByRegion {
                descriptor_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        return WritePlaceShape::Unsupported;
    }
    if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && let Some((pointer_byte_offset, field_byte_offset)) = single_deref_path(target)
    {
        return WritePlaceShape::Pointee {
            pointer_byte_offset,
            field_byte_offset,
        };
    }
    WritePlaceShape::Unsupported
}

/// AArch64 rung for an inline frame 2D array whose two runtime index slots
/// share that same frame base. Each operation family opts into this shape only
/// after its own encoder, replay, relocation, and footprint contracts land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBaseDoubleIndexedShape {
    pub base_byte_offset: usize,
    pub outer_index_offset: usize,
    pub outer_index_byte_size: usize,
    pub outer_stride: usize,
    pub inner_index_offset: usize,
    pub inner_index_byte_size: usize,
    pub inner_stride: usize,
    pub field_byte_offset: usize,
}

/// AArch64 per-operation rung for an inline frame array whose runtime index
/// may live in either storage region. Immediate-integer, exact-binary, and
/// conversion, and string-descriptor writes opt in through separate
/// classifiers; other operation families keep using the narrower shared write
/// classifier until their replay contracts land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBaseIndexedShape {
    pub base_byte_offset: usize,
    pub index_region: omega_target_operations::RuntimeStorageRegion,
    pub index_offset: usize,
    pub index_byte_size: usize,
    pub element_byte_size: usize,
    pub field_byte_offset: usize,
}

pub fn classify_frame_base_indexed_integer_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

pub fn classify_frame_base_indexed_binary_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

pub fn classify_frame_base_indexed_convert_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

pub fn classify_frame_base_indexed_string_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

pub fn classify_frame_base_indexed_address_shape(
    source: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(source)
}

pub fn classify_frame_base_indexed_bounded_buffer_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

pub fn classify_frame_base_indexed_bounded_buffer_literal_append_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

pub fn classify_frame_base_indexed_bounded_buffer_source_append_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    classify_frame_base_indexed_shape(target)
}

fn classify_frame_base_indexed_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseIndexedShape> {
    let indexed = direct_indexed_path(target)?;
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return None;
    }
    Some(FrameBaseIndexedShape {
        base_byte_offset: indexed.pointer_offset,
        index_region: indexed.index_region,
        index_offset: indexed.index_offset,
        index_byte_size: indexed.index_byte_size,
        element_byte_size: indexed.element_byte_size,
        field_byte_offset: indexed.field_offset,
    })
}

pub fn classify_frame_base_double_indexed_binary_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_integer_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_convert_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_string_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_bounded_buffer_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_bounded_buffer_literal_append_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_bounded_buffer_source_append_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_text_assembly_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(target)
}

pub fn classify_frame_base_double_indexed_address_shape(
    source: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    classify_frame_base_double_indexed_shape(source)
}

fn classify_frame_base_double_indexed_shape(
    target: &omega_target_operations::Place,
) -> Option<FrameBaseDoubleIndexedShape> {
    let double = direct_double_indexed_path(target)?;
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    if target.region != frame || double.outer_region != frame || double.inner_region != frame {
        return None;
    }
    Some(FrameBaseDoubleIndexedShape {
        base_byte_offset: double.base_offset,
        outer_index_offset: double.outer_offset,
        outer_index_byte_size: double.outer_byte_size,
        outer_stride: double.outer_stride,
        inner_index_offset: double.inner_offset,
        inner_index_byte_size: double.inner_byte_size,
        inner_stride: double.inner_stride,
        field_byte_offset: double.field_offset,
    })
}

/// Write rung 2a: the place-shaped integer write. x86_64 rides the
/// materializer; aarch64 decomposes only the closed transitional shapes whose
/// retained encoders and final replay contracts have landed.
pub fn encode_write_place_integer(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    value: i64,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) = classify_frame_base_indexed_integer_shape(target)
    {
        return aarch64::encode_runtime_frame_base_indexed_integer_write_with_index_region(
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            byte_size,
            value,
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_double) = classify_frame_base_double_indexed_integer_shape(target)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_integer_write(
            frame_double.base_byte_offset,
            frame_double.outer_index_offset,
            frame_double.outer_index_byte_size,
            frame_double.outer_stride,
            frame_double.inner_index_offset,
            frame_double.inner_index_byte_size,
            frame_double.inner_stride,
            frame_double.field_byte_offset,
            byte_size,
            value,
        );
    }
    match architecture {
        Architecture::X86_64 => {
            x86_64::encode_place_integer_write(target, value, byte_size).map(|(bytes, _)| bytes)
        }
        Architecture::Aarch64 => match classify_write_place_shape(target) {
            WritePlaceShape::Direct { byte_offset } => {
                aarch64::encode_runtime_machine_integer_write(byte_offset, byte_size, value)
            }
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => aarch64::encode_runtime_pointee_integer_write(
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::FrameIndexed {
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_integer_write(
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::FrameIndexedByRegion {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_integer_write_with_index_region(
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_integer_write(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_integer_write(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_integer_write(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::PointeeDoubleIndexed {
                descriptor_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_pointee_double_indexed_integer_write(
                descriptor_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::Unsupported => Err(Diagnostic::error(
                "WritePlaceInteger on aarch64 serves direct, pointee, frame-indexed, \
                 frame-base-indexed, frame-base-double-indexed, machine-indexed, \
                 machine-double-indexed, and pointee-double-indexed \
                 place shapes only until the aarch64 place materializer lands; \
                 this shape refuses loudly",
            )),
        },
    }
}

/// Task #131 (guards consume Places): the place-shaped storage compare.
/// x86_64 rides the materializer; aarch64 serves DIRECT places via the
/// retained storage-compare encoder and refuses walked shapes loudly until
/// its materializer lands.
#[allow(clippy::too_many_arguments)]
pub fn encode_place_compare_bytes(
    architecture: Architecture,
    left: &omega_target_operations::Place,
    right: &omega_target_operations::Place,
    byte_size: usize,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => x86_64::encode_place_compare(
            left,
            right,
            byte_size,
            failure_branch_distance,
            operator,
            is_float,
        )
        .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => match (
            classify_write_place_shape(left),
            classify_write_place_shape(right),
        ) {
            (
                WritePlaceShape::Direct {
                    byte_offset: left_offset,
                },
                WritePlaceShape::Direct {
                    byte_offset: right_offset,
                },
            ) => aarch64::encode_runtime_storage_compare_bytes(
                left_offset,
                right_offset,
                byte_size,
                failure_branch_distance,
                operator,
                is_float,
            ),
            _ => Err(Diagnostic::error(
                "ComparePlaces on aarch64 serves direct place shapes only until the \
                 aarch64 place materializer lands; this shape refuses loudly",
            )),
        },
    }
}

/// One source of truth: the encoder's output length (the branch distance
/// only changes the rel32 payload, never the width).
pub fn place_compare_width(
    architecture: Architecture,
    left: &omega_target_operations::Place,
    right: &omega_target_operations::Place,
    byte_size: usize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<usize, Diagnostic> {
    encode_place_compare_bytes(architecture, left, right, byte_size, 0, operator, is_float)
        .map(|bytes| bytes.len())
}

pub fn x86_64_encode_place_compare_with_sites(
    left: &omega_target_operations::Place,
    right: &omega_target_operations::Place,
    byte_size: usize,
    operator: StateGuardOperator,
    is_float: bool,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_compare(left, right, byte_size, 0, operator, is_float)
}

/// Task #131: the place-vs-immediate compare.
pub fn encode_place_value_compare_bytes(
    architecture: Architecture,
    place: &omega_target_operations::Place,
    byte_size: usize,
    expected_value: i64,
    failure_branch_distance: isize,
    operator: StateGuardOperator,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => x86_64::encode_place_value_compare(
            place,
            byte_size,
            expected_value,
            failure_branch_distance,
            operator,
        )
        .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => match classify_write_place_shape(place) {
            WritePlaceShape::Direct { byte_offset } => {
                aarch64::encode_runtime_storage_value_compare_bytes(
                    byte_offset,
                    byte_size,
                    expected_value,
                    failure_branch_distance,
                    operator,
                )
            }
            _ => Err(Diagnostic::error(
                "ComparePlaceValue on aarch64 serves direct place shapes only until \
                 the aarch64 place materializer lands; this shape refuses loudly",
            )),
        },
    }
}

/// One source of truth: the encoder's output length.
pub fn place_value_compare_width(
    architecture: Architecture,
    place: &omega_target_operations::Place,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> Result<usize, Diagnostic> {
    encode_place_value_compare_bytes(architecture, place, byte_size, expected_value, 0, operator)
        .map(|bytes| bytes.len())
}

pub fn x86_64_encode_place_value_compare_with_sites(
    place: &omega_target_operations::Place,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_value_compare(place, byte_size, expected_value, 0, operator)
}

/// Task #131: a frame-rooted `descriptor deref + scaled index (+ const)`
/// path with ANY index region -- the shape the retired frame-indexed-deref
/// ADDRESS write serves (its index may live in machine storage, which
/// `classify_write_place_shape` deliberately refuses for the VALUE-write
/// families). Both the address encode path and its relocation walker call
/// THIS helper, so the discrimination cannot drift.
pub fn place_frame_deref_indexed_path(
    place: &omega_target_operations::Place,
) -> Option<(
    usize,
    omega_target_operations::RuntimeStorageRegion,
    usize,
    usize,
    usize,
    usize,
)> {
    if place.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return None;
    }
    single_indexed_path(place).map(|indexed| {
        (
            indexed.pointer_offset,
            indexed.index_region,
            indexed.index_offset,
            indexed.index_byte_size,
            indexed.element_byte_size,
            indexed.field_offset,
        )
    })
}

/// Task #131: the place-shaped ADDRESS write (`frame[target] = &place`).
/// x86_64 rides the materializer; aarch64 decomposes to the retained
/// address encoders (direct, pointee -- which also serves the retired
/// FIXED-indexed shape after const folding -- frame-indexed deref with
/// either index region, frame-base-indexed, machine-indexed). Anything
/// else refuses loudly.
pub fn encode_write_place_address(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if architecture == Architecture::Aarch64
        && let Some(double) = classify_frame_base_double_indexed_address_shape(source)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_address_to_runtime_frame_write(
            double.base_byte_offset,
            double.outer_index_offset,
            double.outer_index_byte_size,
            double.outer_stride,
            double.inner_index_offset,
            double.inner_index_byte_size,
            double.inner_stride,
            double.field_byte_offset,
            target_offset,
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) = classify_frame_base_indexed_address_shape(source)
    {
        return aarch64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region(
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            target_offset,
        );
    }
    match architecture {
        Architecture::X86_64 => {
            x86_64::encode_place_address_write(source, target_offset).map(|(bytes, _)| bytes)
        }
        Architecture::Aarch64 => match classify_write_place_shape(source) {
            WritePlaceShape::Direct { byte_offset } => {
                aarch64::encode_runtime_storage_address_to_runtime_frame_write(
                    byte_offset,
                    target_offset,
                )
            }
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => aarch64::encode_runtime_pointee_address_to_runtime_frame_write(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            ),
            WritePlaceShape::FrameIndexed {
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
            ),
            _ => {
                // The machine-index deref shape (classify refuses it for the
                // value writes) has its own retained encoder.
                if let Some((
                    descriptor_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                )) = place_frame_deref_indexed_path(source)
                {
                    return aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                        index_region,
                        descriptor_offset,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                        target_offset,
                    );
                }
                Err(Diagnostic::error(
                    "WritePlaceAddress on aarch64 serves direct, pointee, indexed-deref, \
                     frame-base-indexed, and machine-indexed place shapes only until the \
                     aarch64 place materializer lands; this shape refuses loudly",
                ))
            }
        },
    }
}

pub fn write_place_address_register_writes(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<omega_calling_conventions::RegisterSet, Diagnostic> {
    if architecture == Architecture::Aarch64
        && classify_frame_base_double_indexed_address_shape(source).is_some()
    {
        return Ok(
            aarch64::runtime_frame_base_double_indexed_address_to_runtime_frame_write_clobbers(
                target_offset,
            ),
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) = classify_frame_base_indexed_address_shape(source)
    {
        return Ok(
            aarch64::runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers_with_index_region(
                frame_indexed.index_region,
            ),
        );
    }
    match architecture {
        Architecture::X86_64 => Ok(x86_64::place_address_write_register_writes(source)),
        Architecture::Aarch64 => match classify_write_place_shape(source) {
            WritePlaceShape::Direct { byte_offset } => Ok(
                aarch64::runtime_storage_address_to_runtime_frame_write_clobbers(
                    byte_offset,
                    target_offset,
                ),
            ),
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => Ok(
                aarch64::runtime_pointee_address_to_runtime_frame_write_clobbers(
                    pointer_byte_offset,
                    field_byte_offset,
                    target_offset,
                ),
            ),
            WritePlaceShape::FrameIndexed { .. } => Ok(
                aarch64::runtime_frame_indexed_address_to_runtime_frame_write_clobbers(
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ),
            WritePlaceShape::FrameBaseIndexed { .. } => {
                Ok(aarch64::runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers())
            }
            WritePlaceShape::MachineIndexed { .. } => Ok(
                aarch64::runtime_machine_indexed_address_to_runtime_frame_write_clobbers(
                    target_offset,
                ),
            ),
            WritePlaceShape::MachineDoubleIndexed { .. } => Ok(
                aarch64::runtime_machine_double_indexed_address_to_runtime_frame_write_clobbers(
                    target_offset,
                ),
            ),
            _ => {
                if let Some((_, index_region, ..)) = place_frame_deref_indexed_path(source) {
                    return Ok(
                        aarch64::runtime_frame_indexed_address_to_runtime_frame_write_clobbers(
                            index_region,
                        ),
                    );
                }
                Err(Diagnostic::error(
                    "WritePlaceAddress footprint does not cover a shape its aarch64 encoder refuses",
                ))
            }
        },
    }
}

pub fn write_place_address_additional_machine_state(
    architecture: Architecture,
) -> omega_calling_conventions::MachineStateSet {
    match architecture {
        Architecture::X86_64 => x86_64::place_address_write_additional_machine_state(),
        Architecture::Aarch64 => aarch64::runtime_place_address_write_additional_machine_state(),
    }
}

/// One source of truth: the encoder's output length.
pub fn write_place_address_width(
    architecture: Architecture,
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<usize, Diagnostic> {
    encode_write_place_address(architecture, source, target_offset).map(|bytes| bytes.len())
}

pub fn x86_64_encode_write_place_address_with_sites(
    source: &omega_target_operations::Place,
    target_offset: usize,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_address_write(source, target_offset)
}

/// Text rung 2a: the place-shaped string-descriptor write. x86_64 rides the
/// materializer; aarch64 decomposes by WritePlaceShape to the retained
/// string encoders. Every classified place shape is served; unsupported
/// general paths still refuse loudly until the aarch64 place materializer
/// lands.
pub fn encode_write_place_string(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if architecture == Architecture::Aarch64
        && let Some(frame_double) = classify_frame_base_double_indexed_string_shape(target)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_string_write(
            frame_double.base_byte_offset,
            frame_double.outer_index_offset,
            frame_double.outer_index_byte_size,
            frame_double.outer_stride,
            frame_double.inner_index_offset,
            frame_double.inner_index_byte_size,
            frame_double.inner_stride,
            frame_double.field_byte_offset,
            byte_length,
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) = classify_frame_base_indexed_string_shape(target)
    {
        return aarch64::encode_runtime_frame_base_indexed_string_write_with_index_region(
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            byte_length,
        );
    }
    match architecture {
        Architecture::X86_64 => {
            x86_64::encode_place_string_write(target, byte_length).map(|(bytes, _)| bytes)
        }
        Architecture::Aarch64 => match classify_write_place_shape(target) {
            WritePlaceShape::Direct { byte_offset } => {
                aarch64::encode_runtime_machine_string_write(byte_offset, byte_length)
            }
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => aarch64::encode_runtime_pointee_string_write(
                pointer_byte_offset,
                field_byte_offset,
                byte_length,
            ),
            WritePlaceShape::FrameIndexed {
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_string_write(
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_length,
            ),
            WritePlaceShape::FrameIndexedByRegion {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_string_write_with_index_region(
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_length,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_string_write(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_length,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_string_write_with_index_region(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_length,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_string_write(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                byte_length,
            ),
            _ => Err(Diagnostic::error(
                "WritePlaceString on aarch64 serves every classified place shape; \
                 unsupported general paths refuse loudly until the aarch64 place materializer lands",
            )),
        },
    }
}

/// One source of truth: the encoder's output length.
pub fn write_place_string_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    byte_length: usize,
) -> Result<usize, Diagnostic> {
    encode_write_place_string(architecture, target, byte_length).map(|bytes| bytes.len())
}

pub fn x86_64_encode_write_place_string_with_sites(
    target: &omega_target_operations::Place,
    byte_length: usize,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_string_write(target, byte_length)
}

/// Text rung 2a: the place-shaped bounded-buffer literal write. x86_64 rides
/// the materializer; aarch64 decomposes to retained carrier encoders for every
/// classified place shape.
pub fn encode_write_place_bounded_buffer(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    if architecture == Architecture::Aarch64
        && let Some(frame_double) = classify_frame_base_double_indexed_bounded_buffer_shape(target)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_bounded_buffer_write(
            frame_double.base_byte_offset,
            frame_double.outer_index_offset,
            frame_double.outer_index_byte_size,
            frame_double.outer_stride,
            frame_double.inner_index_offset,
            frame_double.inner_index_byte_size,
            frame_double.inner_stride,
            frame_double.field_byte_offset,
            literal,
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) = classify_frame_base_indexed_bounded_buffer_shape(target)
    {
        return aarch64::encode_runtime_frame_base_indexed_bounded_buffer_write_with_index_region(
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            literal,
        );
    }
    match architecture {
        Architecture::X86_64 => {
            x86_64::encode_place_bounded_buffer_write(target, literal).map(|(bytes, _)| bytes)
        }
        Architecture::Aarch64 => match classify_write_place_shape(target) {
            WritePlaceShape::Direct { byte_offset } => {
                aarch64::encode_runtime_machine_bounded_buffer_write(byte_offset, literal)
            }
            WritePlaceShape::Pointee {
                pointer_byte_offset,
                field_byte_offset,
            } => aarch64::encode_runtime_pointee_bounded_buffer_write(
                pointer_byte_offset,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::FrameIndexed {
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_bounded_buffer_write(
                descriptor_offset,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::FrameIndexedByRegion {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_bounded_buffer_write(
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_bounded_buffer_write(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_bounded_buffer_write(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_bounded_buffer_write(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                literal,
            ),
            _ => Err(Diagnostic::error(
                "WritePlaceBoundedBuffer on aarch64 serves every classified place shape; \
                 unsupported general paths refuse loudly until the aarch64 place materializer lands",
            )),
        },
    }
}

/// One source of truth: the encoder's output length.
pub fn write_place_bounded_buffer_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<usize, Diagnostic> {
    encode_write_place_bounded_buffer(architecture, target, literal).map(|bytes| bytes.len())
}

pub fn x86_64_encode_write_place_bounded_buffer_with_sites(
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_bounded_buffer_write(target, literal)
}

pub fn encode_append_place_bounded_buffer_source(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    source: &omega_target_operations::Place,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => x86_64::encode_place_bounded_buffer_source_append(target, source)
            .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => {
            aarch64_encode_append_place_bounded_buffer_source_with_sites(target, source)
                .map(|(bytes, _)| bytes)
        }
    }
}

pub fn append_place_bounded_buffer_source_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    source: &omega_target_operations::Place,
) -> Result<usize, Diagnostic> {
    encode_append_place_bounded_buffer_source(architecture, target, source).map(|bytes| bytes.len())
}

pub fn encode_append_place_bounded_buffer_literal(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    if architecture == Architecture::Aarch64
        && let Some(frame_double) =
            classify_frame_base_double_indexed_bounded_buffer_literal_append_shape(target)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_bounded_buffer_literal_append(
            frame_double.base_byte_offset,
            frame_double.outer_index_offset,
            frame_double.outer_index_byte_size,
            frame_double.outer_stride,
            frame_double.inner_index_offset,
            frame_double.inner_index_byte_size,
            frame_double.inner_stride,
            frame_double.field_byte_offset,
            literal,
        );
    }
    if architecture == Architecture::Aarch64
        && let Some(frame_indexed) =
            classify_frame_base_indexed_bounded_buffer_literal_append_shape(target)
    {
        return aarch64::encode_runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region(
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            literal,
        );
    }
    match architecture {
        Architecture::X86_64 => x86_64::encode_place_bounded_buffer_literal_append(target, literal)
            .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => match classify_write_place_shape(target) {
            WritePlaceShape::Direct { .. } | WritePlaceShape::Pointee { .. } => {
                aarch64::encode_place_bounded_buffer_literal_append(target, literal)
                    .map(|(bytes, _)| bytes)
            }
            WritePlaceShape::FrameIndexed {
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_bounded_buffer_literal_append(
                descriptor_offset,
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::FrameIndexedByRegion {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_bounded_buffer_literal_append(
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_bounded_buffer_literal_append(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_bounded_buffer_literal_append(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_bounded_buffer_literal_append(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                literal,
            ),
            WritePlaceShape::PointeeDoubleIndexed { .. } | WritePlaceShape::Unsupported => {
                Err(Diagnostic::error(
                    "AppendPlaceBoundedBufferLiteral on aarch64 serves every classified place shape; unsupported general paths refuse loudly until the aarch64 place materializer lands",
                ))
            }
        },
    }
}

pub fn append_place_bounded_buffer_literal_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<usize, Diagnostic> {
    encode_append_place_bounded_buffer_literal(architecture, target, literal)
        .map(|bytes| bytes.len())
}

pub fn x86_64_encode_append_place_bounded_buffer_source_with_sites(
    target: &omega_target_operations::Place,
    source: &omega_target_operations::Place,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_bounded_buffer_source_append(target, source)
}

pub fn x86_64_encode_append_place_bounded_buffer_literal_with_sites(
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_bounded_buffer_literal_append(target, literal)
}

pub fn aarch64_encode_append_place_bounded_buffer_source_with_sites(
    target: &omega_target_operations::Place,
    source: &omega_target_operations::Place,
) -> Result<(Vec<u8>, omega_isa_aarch64::BoundedBufferPlaceSites), Diagnostic> {
    if !matches!(
        classify_write_place_shape(source),
        WritePlaceShape::Direct { .. } | WritePlaceShape::Pointee { .. }
    ) {
        return Err(Diagnostic::error(
            "AppendPlaceBoundedBufferSource on aarch64 requires a direct or pointee source",
        ));
    }
    if let Some(frame_double) =
        classify_frame_base_double_indexed_bounded_buffer_source_append_shape(target)
    {
        return aarch64::encode_runtime_frame_base_double_indexed_bounded_buffer_source_append(
            frame_double.base_byte_offset,
            frame_double.outer_index_offset,
            frame_double.outer_index_byte_size,
            frame_double.outer_stride,
            frame_double.inner_index_offset,
            frame_double.inner_index_byte_size,
            frame_double.inner_stride,
            frame_double.field_byte_offset,
            source,
        );
    }
    if let Some(frame_indexed) =
        classify_frame_base_indexed_bounded_buffer_source_append_shape(target)
    {
        return aarch64::encode_runtime_frame_base_indexed_bounded_buffer_source_append_with_index_region(
            frame_indexed.base_byte_offset,
            frame_indexed.index_region,
            frame_indexed.index_offset,
            frame_indexed.index_byte_size,
            frame_indexed.element_byte_size,
            frame_indexed.field_byte_offset,
            source,
        );
    }
    match classify_write_place_shape(target) {
        WritePlaceShape::Direct { .. } | WritePlaceShape::Pointee { .. } => {
            aarch64::encode_place_bounded_buffer_source_append(target, source)
        }
        WritePlaceShape::FrameIndexed {
            descriptor_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => aarch64::encode_runtime_frame_indexed_bounded_buffer_source_append(
            descriptor_offset,
            omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        WritePlaceShape::FrameIndexedByRegion {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => aarch64::encode_runtime_frame_indexed_bounded_buffer_source_append(
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        WritePlaceShape::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => aarch64::encode_runtime_frame_base_indexed_bounded_buffer_source_append(
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        WritePlaceShape::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        } => aarch64::encode_runtime_machine_indexed_bounded_buffer_source_append(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            source,
        ),
        WritePlaceShape::MachineDoubleIndexed {
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        } => aarch64::encode_runtime_machine_double_indexed_bounded_buffer_source_append(
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            source,
        ),
        WritePlaceShape::PointeeDoubleIndexed { .. } | WritePlaceShape::Unsupported => {
            Err(Diagnostic::error(
                "AppendPlaceBoundedBufferSource on aarch64 retained an unsupported target",
            ))
        }
    }
}

pub fn aarch64_encode_append_place_bounded_buffer_literal_with_sites(
    target: &omega_target_operations::Place,
    literal: &[u8],
) -> Result<(Vec<u8>, omega_isa_aarch64::BoundedBufferPlaceSites), Diagnostic> {
    aarch64::encode_place_bounded_buffer_literal_append(target, literal)
}

/// One source of truth: the encoder's output length.
pub fn write_place_integer_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    value: i64,
    byte_size: usize,
) -> Result<usize, Diagnostic> {
    encode_write_place_integer(architecture, target, value, byte_size).map(|bytes| bytes.len())
}

/// Binary rung 2a: the place-shaped binary write. x86_64 rides the
/// materializer; aarch64 decomposes by WritePlaceShape to the retained
/// binary encoders. The SHAPED aarch64 encoders are Exact-only (matching
/// today's producer split); a non-Exact/float shaped place refuses loudly.
#[allow(clippy::too_many_arguments)]
pub fn encode_write_place_binary(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &omega_target_operations::Place,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => x86_64::encode_place_binary_write(
            runtime_value_operands,
            target,
            byte_size,
            left,
            operator,
            right,
            is_float,
            domain,
            target_signed,
        )
        .map(|(bytes, _)| bytes),
        Architecture::Aarch64 => {
            let shape = classify_write_place_shape(target);
            let frame_indexed = classify_frame_base_indexed_binary_shape(target);
            let frame_double = classify_frame_base_double_indexed_binary_shape(target);
            if (frame_indexed.is_some()
                || frame_double.is_some()
                || !matches!(shape, WritePlaceShape::Direct { .. }))
                && (is_float || domain != psi_numerics::arithmetic::ArithmeticDomain::Exact)
            {
                return Err(Diagnostic::error(
                    "WritePlaceBinary on aarch64: shaped (deref/indexed) targets \
                     serve Exact integer domains only until the aarch64 place \
                     materializer lands",
                ));
            }
            if let Some(frame_indexed) = frame_indexed {
                return aarch64::encode_runtime_frame_base_indexed_binary_write_with_index_region(
                    runtime_value_operands,
                    frame_indexed.base_byte_offset,
                    frame_indexed.index_region,
                    frame_indexed.index_offset,
                    frame_indexed.index_byte_size,
                    frame_indexed.element_byte_size,
                    frame_indexed.field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                );
            }
            if let Some(frame_double) = frame_double {
                return aarch64::encode_runtime_frame_base_double_indexed_binary_write(
                    runtime_value_operands,
                    frame_double.base_byte_offset,
                    frame_double.outer_index_offset,
                    frame_double.outer_index_byte_size,
                    frame_double.outer_stride,
                    frame_double.inner_index_offset,
                    frame_double.inner_index_byte_size,
                    frame_double.inner_stride,
                    frame_double.field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                );
            }
            match shape {
                WritePlaceShape::Direct { byte_offset } => {
                    aarch64::encode_runtime_storage_binary_write(
                        runtime_value_operands,
                        byte_offset,
                        byte_size,
                        left,
                        operator,
                        right,
                        is_float,
                        domain,
                        target_signed,
                    )
                }
                WritePlaceShape::Pointee {
                    pointer_byte_offset,
                    field_byte_offset,
                } => aarch64::encode_runtime_pointee_binary_write(
                    runtime_value_operands,
                    pointer_byte_offset,
                    field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                ),
                WritePlaceShape::FrameIndexed {
                    descriptor_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_frame_indexed_binary_write(
                    runtime_value_operands,
                    descriptor_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                ),
                WritePlaceShape::FrameIndexedByRegion {
                    descriptor_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_frame_indexed_binary_write_with_index_region(
                    runtime_value_operands,
                    descriptor_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                ),
                WritePlaceShape::FrameBaseIndexed {
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_frame_base_indexed_binary_write(
                    runtime_value_operands,
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                ),
                WritePlaceShape::MachineIndexed {
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_machine_indexed_binary_write(
                    runtime_value_operands,
                    base_byte_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                ),
                WritePlaceShape::MachineDoubleIndexed {
                    base_byte_offset,
                    outer_index_region,
                    outer_index_offset,
                    outer_index_byte_size,
                    outer_stride,
                    inner_index_region,
                    inner_index_offset,
                    inner_index_byte_size,
                    inner_stride,
                    field_byte_offset,
                } => aarch64::encode_runtime_machine_double_indexed_binary_write(
                    runtime_value_operands,
                    base_byte_offset,
                    outer_index_offset,
                    outer_index_region,
                    outer_index_byte_size,
                    outer_stride,
                    inner_index_offset,
                    inner_index_region,
                    inner_index_byte_size,
                    inner_stride,
                    field_byte_offset,
                    byte_size,
                    left,
                    operator,
                    right,
                ),
                WritePlaceShape::PointeeDoubleIndexed { .. } | WritePlaceShape::Unsupported => {
                    Err(Diagnostic::error(
                        "WritePlaceBinary on aarch64 serves direct, pointee, frame-indexed, \
                     cross-region frame-indexed, frame-base-indexed, machine-indexed, and \
                     machine-double-indexed place shapes only until the aarch64 place \
                     materializer lands",
                    ))
                }
            }
        }
    }
}

/// One source of truth: the encoder's output length.
#[allow(clippy::too_many_arguments)]
pub fn write_place_binary_width(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &omega_target_operations::Place,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> Result<usize, Diagnostic> {
    encode_write_place_binary(
        architecture,
        runtime_value_operands,
        target,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    )
    .map(|bytes| bytes.len())
}

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
            CopyPlacesShape::FromPointeeDoubleIndexed {
                descriptor_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_pointee_double_indexed_to_runtime_storage(
                descriptor_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target.region,
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
            // The runtime-indexed decomposes: the descriptor is frame-held;
            // the index may live in frame or machine storage. The place
            // regions must still match the retained copy encoders.
            CopyPlacesShape::FromIndexed {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                match target.region {
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                        aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_with_index_region(
                            descriptor_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            target_offset,
                            byte_count,
                        )
                    }
                    omega_target_operations::RuntimeStorageRegion::Machine => {
                        aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_with_index_region(
                            descriptor_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
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
                index_byte_size,
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
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    byte_count,
                )
            }
            CopyPlacesShape::ToIndexedByRegion {
                source_offset,
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } if target.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                aarch64::encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
                    source.region,
                    source_offset,
                    descriptor_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    byte_count,
                )
            }
            CopyPlacesShape::IndexedToPointee {
                descriptor_offset,
                index_offset,
                index_byte_size,
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
                    index_byte_size,
                    element_byte_size,
                    source_field_byte_offset,
                    pointer_byte_offset,
                    target_field_byte_offset,
                    byte_count,
                )
            }
            CopyPlacesShape::IndexedToPointeeByRegion {
                descriptor_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            } if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                aarch64::encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region(
                    descriptor_offset,
                    index_region,
                    index_offset,
                    index_byte_size,
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
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
                base_byte_offset,
                index_offset,
                index_region,
                index_byte_size,
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
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
                source_offset,
                base_byte_offset,
                index_offset,
                index_region,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::MachineIndexedToPointee {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_machine_indexed_to_runtime_pointee(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::PointeeToMachineIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_runtime_pointee_to_machine_indexed(
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FromFrameBaseIndexed {
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame(
                base_byte_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            CopyPlacesShape::ToFrameBaseIndexed {
                source_offset,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage(
                source.region,
                source_offset,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FrameBaseIndexedToPointee {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee(
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::PointeeToFrameBaseIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed(
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FromMachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            CopyPlacesShape::FromFrameBaseDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            CopyPlacesShape::FrameBaseDoubleIndexedToPointee {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee(
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::PointeeToFrameBaseDoubleIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed(
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::MachineDoubleIndexedToPointee {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_machine_double_indexed_to_runtime_pointee(
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                source_field_byte_offset,
                pointer_byte_offset,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::PointeeToMachineDoubleIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_runtime_pointee_to_machine_double_indexed(
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::ToFrameBaseDoubleIndexed {
                source_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage(
                source.region,
                source_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::ToMachineDoubleIndexed {
                source_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_index_byte_size,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
                source.region,
                source_offset,
                base_byte_offset,
                outer_index_offset,
                outer_index_region,
                outer_index_byte_size,
                outer_stride,
                inner_index_offset,
                inner_index_region,
                inner_index_byte_size,
                inner_stride,
                field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::MachineIndexedPair {
                source_base_byte_offset,
                source_index_region,
                source_index_offset,
                source_index_byte_size,
                source_element_byte_size,
                source_field_byte_offset,
                target_base_byte_offset,
                target_index_region,
                target_index_offset,
                target_index_byte_size,
                target_element_byte_size,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
                source_base_byte_offset,
                source_index_offset,
                source_index_region,
                source_index_byte_size,
                source_element_byte_size,
                source_field_byte_offset,
                target_base_byte_offset,
                target_index_offset,
                target_index_region,
                target_index_byte_size,
                target_element_byte_size,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FrameBaseIndexedPair {
                source_base_byte_offset,
                source_index_region,
                source_index_offset,
                source_index_byte_size,
                source_element_byte_size,
                source_field_byte_offset,
                target_base_byte_offset,
                target_index_region,
                target_index_offset,
                target_index_byte_size,
                target_element_byte_size,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_frame_base_indexed_to_frame_base_indexed(
                source_base_byte_offset,
                source_index_region,
                source_index_offset,
                source_index_byte_size,
                source_element_byte_size,
                source_field_byte_offset,
                target_base_byte_offset,
                target_index_region,
                target_index_offset,
                target_index_byte_size,
                target_element_byte_size,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::CrossRegionIndexedPair {
                source_base_byte_offset,
                source_index_region,
                source_index_offset,
                source_index_byte_size,
                source_element_byte_size,
                source_field_byte_offset,
                target_base_byte_offset,
                target_index_region,
                target_index_offset,
                target_index_byte_size,
                target_element_byte_size,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_cross_region_indexed_pair(
                source.region,
                source_base_byte_offset,
                source_index_region,
                source_index_offset,
                source_index_byte_size,
                source_element_byte_size,
                source_field_byte_offset,
                target.region,
                target_base_byte_offset,
                target_index_region,
                target_index_offset,
                target_index_byte_size,
                target_element_byte_size,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::CrossRegionDoubleIndexedPair {
                source_base_byte_offset,
                source_outer_index_region,
                source_outer_index_offset,
                source_outer_index_byte_size,
                source_outer_stride,
                source_inner_index_region,
                source_inner_index_offset,
                source_inner_index_byte_size,
                source_inner_stride,
                source_field_byte_offset,
                target_base_byte_offset,
                target_outer_index_region,
                target_outer_index_offset,
                target_outer_index_byte_size,
                target_outer_stride,
                target_inner_index_region,
                target_inner_index_offset,
                target_inner_index_byte_size,
                target_inner_stride,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_cross_region_double_indexed_pair(
                source.region,
                source_base_byte_offset,
                source_outer_index_region,
                source_outer_index_offset,
                source_outer_index_byte_size,
                source_outer_stride,
                source_inner_index_region,
                source_inner_index_offset,
                source_inner_index_byte_size,
                source_inner_stride,
                source_field_byte_offset,
                target.region,
                target_base_byte_offset,
                target_outer_index_region,
                target_outer_index_offset,
                target_outer_index_byte_size,
                target_outer_stride,
                target_inner_index_region,
                target_inner_index_offset,
                target_inner_index_byte_size,
                target_inner_stride,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::FrameBaseDoubleIndexedPair {
                source_base_byte_offset,
                source_outer_index_region,
                source_outer_index_offset,
                source_outer_index_byte_size,
                source_outer_stride,
                source_inner_index_region,
                source_inner_index_offset,
                source_inner_index_byte_size,
                source_inner_stride,
                source_field_byte_offset,
                target_base_byte_offset,
                target_outer_index_region,
                target_outer_index_offset,
                target_outer_index_byte_size,
                target_outer_stride,
                target_inner_index_region,
                target_inner_index_offset,
                target_inner_index_byte_size,
                target_inner_stride,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed(
                source_base_byte_offset,
                source_outer_index_region,
                source_outer_index_offset,
                source_outer_index_byte_size,
                source_outer_stride,
                source_inner_index_region,
                source_inner_index_offset,
                source_inner_index_byte_size,
                source_inner_stride,
                source_field_byte_offset,
                target_base_byte_offset,
                target_outer_index_region,
                target_outer_index_offset,
                target_outer_index_byte_size,
                target_outer_stride,
                target_inner_index_region,
                target_inner_index_offset,
                target_inner_index_byte_size,
                target_inner_stride,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::MachineDoubleIndexedPair {
                source_base_byte_offset,
                source_outer_index_region,
                source_outer_index_offset,
                source_outer_index_byte_size,
                source_outer_stride,
                source_inner_index_region,
                source_inner_index_offset,
                source_inner_index_byte_size,
                source_inner_stride,
                source_field_byte_offset,
                target_base_byte_offset,
                target_outer_index_region,
                target_outer_index_offset,
                target_outer_index_byte_size,
                target_outer_stride,
                target_inner_index_region,
                target_inner_index_offset,
                target_inner_index_byte_size,
                target_inner_stride,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_machine_double_indexed_to_machine_double_indexed(
                source_base_byte_offset,
                source_outer_index_region,
                source_outer_index_offset,
                source_outer_index_byte_size,
                source_outer_stride,
                source_inner_index_region,
                source_inner_index_offset,
                source_inner_index_byte_size,
                source_inner_stride,
                source_field_byte_offset,
                target_base_byte_offset,
                target_outer_index_region,
                target_outer_index_offset,
                target_outer_index_byte_size,
                target_outer_stride,
                target_inner_index_region,
                target_inner_index_offset,
                target_inner_index_byte_size,
                target_inner_stride,
                target_field_byte_offset,
                byte_count,
            ),
            CopyPlacesShape::PointeePair { .. }
            | CopyPlacesShape::FromIndexed { .. }
            | CopyPlacesShape::ToIndexed { .. }
            | CopyPlacesShape::ToIndexedByRegion { .. }
            | CopyPlacesShape::IndexedToPointee { .. }
            | CopyPlacesShape::IndexedToPointeeByRegion { .. }
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
    /// A frame-held pointer followed by two runtime indices, copied into one
    /// direct frame or machine storage place.
    FromPointeeDoubleIndexed {
        descriptor_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
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
    /// from-frame-indexed copies. The descriptor is frame-held; the index
    /// region is retained explicitly.
    FromIndexed {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
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
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    /// Direct storage copied through a frame-held descriptor when either the
    /// direct source or the dynamic index uses a distinct storage region.
    ToIndexedByRegion {
        source_offset: usize,
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    /// Runtime-indexed source landing through a pointer slot.
    IndexedToPointee {
        descriptor_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    IndexedToPointeeByRegion {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
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
        index_byte_size: usize,
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
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineIndexedToPointee {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    PointeeToMachineIndexed {
        pointer_byte_offset: usize,
        source_field_byte_offset: usize,
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        target_field_byte_offset: usize,
    },
    /// A FRAME-resident inline-array element read into a frame slot (the
    /// retired frame-base-indexed copy): all-frame, single index, no deref.
    FromFrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// A direct storage slot written into a frame-resident inline array. The
    /// runtime index retains its own storage region.
    ToFrameBaseIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    /// An all-frame inline-array element copied through a frame-held target
    /// pointer. The array, index, and pointer slot share one frame base.
    FrameBaseIndexedToPointee {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    /// A frame-held source pointee copied into an all-frame inline-array
    /// element. The pointer slot, array, and index share one frame base.
    PointeeToFrameBaseIndexed {
        pointer_byte_offset: usize,
        source_field_byte_offset: usize,
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        target_field_byte_offset: usize,
    },
    /// A MACHINE inline 2D-array element read (`m[i][j]` -- no deref):
    /// the double-indexed copy. Index-slot regions vary per index.
    FromMachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// A FRAME inline 2D-array element read into direct storage. Each runtime
    /// index retains its own storage region.
    FromFrameBaseDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// A frame-inline 2D-array element copied through a frame-held target
    /// pointer. The array, pointer slot, and frame-held indices share one
    /// frame base; machine-held indices share one additional machine base.
    FrameBaseDoubleIndexedToPointee {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    /// A frame-held source pointee copied into a frame-inline 2D-array element.
    /// The pointer slot, array, and frame-held indices share one frame base;
    /// machine-held indices share one additional machine base.
    PointeeToFrameBaseDoubleIndexed {
        pointer_byte_offset: usize,
        source_field_byte_offset: usize,
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        target_field_byte_offset: usize,
    },
    /// A machine-rooted inline 2D element copied through a frame-held target
    /// pointer. Each runtime index retains its own storage region.
    MachineDoubleIndexedToPointee {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
    },
    /// A frame-held source pointee copied into a machine-rooted inline 2D
    /// element. Each runtime index retains its own storage region.
    PointeeToMachineDoubleIndexed {
        pointer_byte_offset: usize,
        source_field_byte_offset: usize,
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        target_field_byte_offset: usize,
    },
    /// A direct storage slot written into a frame-inline 2D-array element.
    /// Each runtime index retains its own storage region.
    ToFrameBaseDoubleIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    /// The machine inline 2D-array element WRITE (`m[i][j] = v` -- a
    /// const-offset source into a double-indexed machine target).
    ToMachineDoubleIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    /// `arr[i] = arr[j]` on machine inline arrays: ONE runtime index per
    /// side, both sides machine-resident, no deref.
    MachineIndexedPair {
        source_base_byte_offset: usize,
        source_index_region: omega_target_operations::RuntimeStorageRegion,
        source_index_offset: usize,
        source_index_byte_size: usize,
        source_element_byte_size: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_index_region: omega_target_operations::RuntimeStorageRegion,
        target_index_offset: usize,
        target_index_byte_size: usize,
        target_element_byte_size: usize,
        target_field_byte_offset: usize,
    },
    /// `arr[i] = arr[j]` on frame-inline arrays. Each runtime index retains its
    /// own storage region.
    FrameBaseIndexedPair {
        source_base_byte_offset: usize,
        source_index_region: omega_target_operations::RuntimeStorageRegion,
        source_index_offset: usize,
        source_index_byte_size: usize,
        source_element_byte_size: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_index_region: omega_target_operations::RuntimeStorageRegion,
        target_index_offset: usize,
        target_index_byte_size: usize,
        target_element_byte_size: usize,
        target_field_byte_offset: usize,
    },
    /// `target[j] = source[i]` across one machine-inline and one frame-inline
    /// array. Each runtime index retains its own storage region.
    CrossRegionIndexedPair {
        source_base_byte_offset: usize,
        source_index_region: omega_target_operations::RuntimeStorageRegion,
        source_index_offset: usize,
        source_index_byte_size: usize,
        source_element_byte_size: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_index_region: omega_target_operations::RuntimeStorageRegion,
        target_index_offset: usize,
        target_index_byte_size: usize,
        target_element_byte_size: usize,
        target_field_byte_offset: usize,
    },
    /// Double-indexed aggregate copy across one machine-inline and one
    /// frame-inline 2D array. All four indices retain their storage regions.
    CrossRegionDoubleIndexedPair {
        source_base_byte_offset: usize,
        source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
        source_outer_index_offset: usize,
        source_outer_index_byte_size: usize,
        source_outer_stride: usize,
        source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
        source_inner_index_offset: usize,
        source_inner_index_byte_size: usize,
        source_inner_stride: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
        target_outer_index_offset: usize,
        target_outer_index_byte_size: usize,
        target_outer_stride: usize,
        target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
        target_inner_index_offset: usize,
        target_inner_index_byte_size: usize,
        target_inner_stride: usize,
        target_field_byte_offset: usize,
    },
    /// `grid[a][b] = grid[i][j]` on frame-inline 2D arrays. Each runtime
    /// index retains its own storage region.
    FrameBaseDoubleIndexedPair {
        source_base_byte_offset: usize,
        source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
        source_outer_index_offset: usize,
        source_outer_index_byte_size: usize,
        source_outer_stride: usize,
        source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
        source_inner_index_offset: usize,
        source_inner_index_byte_size: usize,
        source_inner_stride: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
        target_outer_index_offset: usize,
        target_outer_index_byte_size: usize,
        target_outer_stride: usize,
        target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
        target_inner_index_offset: usize,
        target_inner_index_byte_size: usize,
        target_inner_stride: usize,
        target_field_byte_offset: usize,
    },
    /// `grid[a][b] = grid[i][j]` on machine-rooted inline 2D arrays. Each
    /// runtime index retains its own frame-or-machine storage region.
    MachineDoubleIndexedPair {
        source_base_byte_offset: usize,
        source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
        source_outer_index_offset: usize,
        source_outer_index_byte_size: usize,
        source_outer_stride: usize,
        source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
        source_inner_index_offset: usize,
        source_inner_index_byte_size: usize,
        source_inner_stride: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
        target_outer_index_offset: usize,
        target_outer_index_byte_size: usize,
        target_outer_stride: usize,
        target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
        target_inner_index_offset: usize,
        target_inner_index_byte_size: usize,
        target_inner_stride: usize,
        target_field_byte_offset: usize,
    },
    /// Anything else (multi-index, multi-deref): x86_64-materializer only.
    General,
}

pub fn classify_copy_places_shape(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> CopyPlacesShape {
    if let Some(double) = pointee_double_indexed_path(source)
        && let Some(target_offset) = target.const_offset()
    {
        return CopyPlacesShape::FromPointeeDoubleIndexed {
            descriptor_offset: double.descriptor_offset,
            outer_index_region: double.outer_region,
            outer_index_offset: double.outer_offset,
            outer_index_byte_size: double.outer_byte_size,
            outer_stride: double.outer_stride,
            inner_index_region: double.inner_region,
            inner_index_offset: double.inner_offset,
            inner_index_byte_size: double.inner_byte_size,
            inner_stride: double.inner_stride,
            field_byte_offset: double.field_offset,
            target_offset,
        };
    }
    // MACHINE inline-array shapes first (no deref -- the array lives in
    // machine statics): the index slot's region rides the ScaledIndex step.
    // A FRAME-rooted no-deref indexed place (the FrameBaseIndexed family)
    // stays General until its rung.
    // The DOUBLE-indexed inline 2D-array reads first (a double path is
    // never a single path -- the recognizers refuse each other's shapes).
    if let Some(double) = direct_double_indexed_path(source) {
        if let Some(target_double) = direct_double_indexed_path(target)
            && source.region == omega_target_operations::RuntimeStorageRegion::Machine
            && target.region == omega_target_operations::RuntimeStorageRegion::Machine
        {
            return CopyPlacesShape::MachineDoubleIndexedPair {
                source_base_byte_offset: double.base_offset,
                source_outer_index_region: double.outer_region,
                source_outer_index_offset: double.outer_offset,
                source_outer_index_byte_size: double.outer_byte_size,
                source_outer_stride: double.outer_stride,
                source_inner_index_region: double.inner_region,
                source_inner_index_offset: double.inner_offset,
                source_inner_index_byte_size: double.inner_byte_size,
                source_inner_stride: double.inner_stride,
                source_field_byte_offset: double.field_offset,
                target_base_byte_offset: target_double.base_offset,
                target_outer_index_region: target_double.outer_region,
                target_outer_index_offset: target_double.outer_offset,
                target_outer_index_byte_size: target_double.outer_byte_size,
                target_outer_stride: target_double.outer_stride,
                target_inner_index_region: target_double.inner_region,
                target_inner_index_offset: target_double.inner_offset,
                target_inner_index_byte_size: target_double.inner_byte_size,
                target_inner_stride: target_double.inner_stride,
                target_field_byte_offset: target_double.field_offset,
            };
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::Machine
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
        {
            return CopyPlacesShape::MachineDoubleIndexedToPointee {
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                source_field_byte_offset: double.field_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            };
        }
        if let Some(target_double) = direct_double_indexed_path(target)
            && source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        {
            return CopyPlacesShape::FrameBaseDoubleIndexedPair {
                source_base_byte_offset: double.base_offset,
                source_outer_index_region: double.outer_region,
                source_outer_index_offset: double.outer_offset,
                source_outer_index_byte_size: double.outer_byte_size,
                source_outer_stride: double.outer_stride,
                source_inner_index_region: double.inner_region,
                source_inner_index_offset: double.inner_offset,
                source_inner_index_byte_size: double.inner_byte_size,
                source_inner_stride: double.inner_stride,
                source_field_byte_offset: double.field_offset,
                target_base_byte_offset: target_double.base_offset,
                target_outer_index_region: target_double.outer_region,
                target_outer_index_offset: target_double.outer_offset,
                target_outer_index_byte_size: target_double.outer_byte_size,
                target_outer_stride: target_double.outer_stride,
                target_inner_index_region: target_double.inner_region,
                target_inner_index_offset: target_double.inner_offset,
                target_inner_index_byte_size: target_double.inner_byte_size,
                target_inner_stride: target_double.inner_stride,
                target_field_byte_offset: target_double.field_offset,
            };
        }
        if let Some(target_double) = direct_double_indexed_path(target)
            && source.region != target.region
        {
            return CopyPlacesShape::CrossRegionDoubleIndexedPair {
                source_base_byte_offset: double.base_offset,
                source_outer_index_region: double.outer_region,
                source_outer_index_offset: double.outer_offset,
                source_outer_index_byte_size: double.outer_byte_size,
                source_outer_stride: double.outer_stride,
                source_inner_index_region: double.inner_region,
                source_inner_index_offset: double.inner_offset,
                source_inner_index_byte_size: double.inner_byte_size,
                source_inner_stride: double.inner_stride,
                source_field_byte_offset: double.field_offset,
                target_base_byte_offset: target_double.base_offset,
                target_outer_index_region: target_double.outer_region,
                target_outer_index_offset: target_double.outer_offset,
                target_outer_index_byte_size: target_double.outer_byte_size,
                target_outer_stride: target_double.outer_stride,
                target_inner_index_region: target_double.inner_region,
                target_inner_index_offset: target_double.inner_offset,
                target_inner_index_byte_size: target_double.inner_byte_size,
                target_inner_stride: target_double.inner_stride,
                target_field_byte_offset: target_double.field_offset,
            };
        }
        if let Some(target_offset) = target.const_offset() {
            if source.region == omega_target_operations::RuntimeStorageRegion::Machine {
                return CopyPlacesShape::FromMachineDoubleIndexed {
                    base_byte_offset: double.base_offset,
                    outer_index_region: double.outer_region,
                    outer_index_offset: double.outer_offset,
                    outer_index_byte_size: double.outer_byte_size,
                    outer_stride: double.outer_stride,
                    inner_index_region: double.inner_region,
                    inner_index_offset: double.inner_offset,
                    inner_index_byte_size: double.inner_byte_size,
                    inner_stride: double.inner_stride,
                    field_byte_offset: double.field_offset,
                    target_offset,
                };
            }
            // Any const-offset target serves: the retained encoder is
            // ..._to_runtime_storage and the walker patches the target
            // base by its own region.
            if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                return CopyPlacesShape::FromFrameBaseDoubleIndexed {
                    base_byte_offset: double.base_offset,
                    outer_index_region: double.outer_region,
                    outer_index_offset: double.outer_offset,
                    outer_index_byte_size: double.outer_byte_size,
                    outer_stride: double.outer_stride,
                    inner_index_region: double.inner_region,
                    inner_index_offset: double.inner_offset,
                    inner_index_byte_size: double.inner_byte_size,
                    inner_stride: double.inner_stride,
                    field_byte_offset: double.field_offset,
                    target_offset,
                };
            }
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
        {
            return CopyPlacesShape::FrameBaseDoubleIndexedToPointee {
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                source_field_byte_offset: double.field_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    if let Some(double) = direct_double_indexed_path(target) {
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some((pointer_byte_offset, source_field_byte_offset)) = single_deref_path(source)
        {
            return CopyPlacesShape::PointeeToMachineDoubleIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                target_field_byte_offset: double.field_offset,
            };
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, source_field_byte_offset)) = single_deref_path(source)
        {
            return CopyPlacesShape::PointeeToFrameBaseDoubleIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                target_field_byte_offset: double.field_offset,
            };
        }
        if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToFrameBaseDoubleIndexed {
                source_offset,
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                field_byte_offset: double.field_offset,
            };
        }
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToMachineDoubleIndexed {
                source_offset,
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_index_byte_size: double.outer_byte_size,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_index_byte_size: double.inner_byte_size,
                inner_stride: double.inner_stride,
                field_byte_offset: double.field_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    if let Some(indexed) = direct_indexed_path(source) {
        if let Some(target_indexed) = direct_indexed_path(target) {
            if source.region == omega_target_operations::RuntimeStorageRegion::Machine
                && target.region == omega_target_operations::RuntimeStorageRegion::Machine
            {
                return CopyPlacesShape::MachineIndexedPair {
                    source_base_byte_offset: indexed.pointer_offset,
                    source_index_region: indexed.index_region,
                    source_index_offset: indexed.index_offset,
                    source_index_byte_size: indexed.index_byte_size,
                    source_element_byte_size: indexed.element_byte_size,
                    source_field_byte_offset: indexed.field_offset,
                    target_base_byte_offset: target_indexed.pointer_offset,
                    target_index_region: target_indexed.index_region,
                    target_index_offset: target_indexed.index_offset,
                    target_index_byte_size: target_indexed.index_byte_size,
                    target_element_byte_size: target_indexed.element_byte_size,
                    target_field_byte_offset: target_indexed.field_offset,
                };
            }
            if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                return CopyPlacesShape::FrameBaseIndexedPair {
                    source_base_byte_offset: indexed.pointer_offset,
                    source_index_region: indexed.index_region,
                    source_index_offset: indexed.index_offset,
                    source_index_byte_size: indexed.index_byte_size,
                    source_element_byte_size: indexed.element_byte_size,
                    source_field_byte_offset: indexed.field_offset,
                    target_base_byte_offset: target_indexed.pointer_offset,
                    target_index_region: target_indexed.index_region,
                    target_index_offset: target_indexed.index_offset,
                    target_index_byte_size: target_indexed.index_byte_size,
                    target_element_byte_size: target_indexed.element_byte_size,
                    target_field_byte_offset: target_indexed.field_offset,
                };
            }
            if source.region != target.region {
                return CopyPlacesShape::CrossRegionIndexedPair {
                    source_base_byte_offset: indexed.pointer_offset,
                    source_index_region: indexed.index_region,
                    source_index_offset: indexed.index_offset,
                    source_index_byte_size: indexed.index_byte_size,
                    source_element_byte_size: indexed.element_byte_size,
                    source_field_byte_offset: indexed.field_offset,
                    target_base_byte_offset: target_indexed.pointer_offset,
                    target_index_region: target_indexed.index_region,
                    target_index_offset: target_indexed.index_offset,
                    target_index_byte_size: target_indexed.index_byte_size,
                    target_element_byte_size: target_indexed.element_byte_size,
                    target_field_byte_offset: target_indexed.field_offset,
                };
            }
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::Machine
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
        {
            return CopyPlacesShape::MachineIndexedToPointee {
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                source_field_byte_offset: indexed.field_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            };
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some(target_offset) = target.const_offset()
        {
            return CopyPlacesShape::FromMachineIndexed {
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
                target_offset,
            };
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
        {
            return CopyPlacesShape::FrameBaseIndexedToPointee {
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                source_field_byte_offset: indexed.field_offset,
                pointer_byte_offset,
                target_field_byte_offset,
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
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
                target_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    if let Some(indexed) = direct_indexed_path(target) {
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some((pointer_byte_offset, source_field_byte_offset)) = single_deref_path(source)
        {
            return CopyPlacesShape::PointeeToMachineIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                target_field_byte_offset: indexed.field_offset,
            };
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, source_field_byte_offset)) = single_deref_path(source)
        {
            return CopyPlacesShape::PointeeToFrameBaseIndexed {
                pointer_byte_offset,
                source_field_byte_offset,
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                target_field_byte_offset: indexed.field_offset,
            };
        }
        if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToFrameBaseIndexed {
                source_offset,
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToMachineIndexed {
                source_offset,
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    // The indexed shapes first: an indexed path is NOT a single-deref path,
    // so these never shadow the pointee arms below. Frame-resident index
    // slots may live in frame or machine storage; the shared address helper
    // materializes the distinct index base when required.
    if let Some(indexed) = single_indexed_path(source) {
        if let Some(target_offset) = target.const_offset() {
            return CopyPlacesShape::FromIndexed {
                descriptor_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
                target_offset,
            };
        }
        if indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            if let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
            {
                return CopyPlacesShape::IndexedToPointee {
                    descriptor_offset: indexed.pointer_offset,
                    index_offset: indexed.index_offset,
                    index_byte_size: indexed.index_byte_size,
                    element_byte_size: indexed.element_byte_size,
                    source_field_byte_offset: indexed.field_offset,
                    pointer_byte_offset,
                    target_field_byte_offset,
                };
            }
        }
        if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some((pointer_byte_offset, target_field_byte_offset)) = single_deref_path(target)
        {
            return CopyPlacesShape::IndexedToPointeeByRegion {
                descriptor_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                source_field_byte_offset: indexed.field_offset,
                pointer_byte_offset,
                target_field_byte_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    if let Some(indexed) = single_indexed_path(target) {
        if indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToIndexed {
                source_offset,
                descriptor_offset: indexed.pointer_offset,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToIndexedByRegion {
                source_offset,
                descriptor_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
                index_byte_size: indexed.index_byte_size,
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_offset: usize,
}

/// A DIRECT indexed hop (no deref -- the inline-array shape):
/// `[ConstOffset(base)?, ScaledIndex, ConstOffset(field)?]`.
fn direct_indexed_path(place: &omega_target_operations::Place) -> Option<SingleIndexedPath> {
    let mut steps = place.steps().iter();
    let mut pointer_offset = 0usize;
    let (index_region, index_offset, index_byte_size, element_byte_size) = loop {
        match steps.next() {
            Some(omega_target_operations::PlaceStep::ConstOffset(offset)) => {
                pointer_offset += offset
            }
            Some(omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            }) => {
                break (
                    *index_region,
                    *index_offset,
                    *index_byte_size,
                    *element_byte_size,
                );
            }
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
        index_byte_size,
        element_byte_size,
        field_offset,
    })
}

struct DoubleIndexedPath {
    base_offset: usize,
    outer_region: omega_target_operations::RuntimeStorageRegion,
    outer_offset: usize,
    outer_byte_size: usize,
    outer_stride: usize,
    inner_region: omega_target_operations::RuntimeStorageRegion,
    inner_offset: usize,
    inner_byte_size: usize,
    inner_stride: usize,
    field_offset: usize,
}

struct PointeeDoubleIndexedPath {
    descriptor_offset: usize,
    outer_region: omega_target_operations::RuntimeStorageRegion,
    outer_offset: usize,
    outer_byte_size: usize,
    outer_stride: usize,
    inner_region: omega_target_operations::RuntimeStorageRegion,
    inner_offset: usize,
    inner_byte_size: usize,
    inner_stride: usize,
    field_offset: usize,
}

/// `Const*, Deref, Const*/SI/Const*/SI/Const*`: a frame-held pointer followed
/// by exactly two scaled indices. Every constant after the deref contributes
/// to the pointee-relative field offset because address addition commutes;
/// any second deref or third index refuses.
fn pointee_double_indexed_path(
    place: &omega_target_operations::Place,
) -> Option<PointeeDoubleIndexedPath> {
    if place.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return None;
    }
    let mut descriptor_offset = 0usize;
    let mut dereferenced = false;
    let mut field_offset = 0usize;
    let mut indices = Vec::new();
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if !dereferenced => {
                descriptor_offset = descriptor_offset.checked_add(*offset)?;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_offset = field_offset.checked_add(*offset)?;
            }
            omega_target_operations::PlaceStep::Deref if !dereferenced => {
                dereferenced = true;
            }
            omega_target_operations::PlaceStep::Deref => return None,
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if dereferenced && indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            omega_target_operations::PlaceStep::ScaledIndex { .. } => return None,
        }
    }
    if !dereferenced {
        return None;
    }
    let [
        (outer_region, outer_offset, outer_byte_size, outer_stride),
        (inner_region, inner_offset, inner_byte_size, inner_stride),
    ] = indices[..]
    else {
        return None;
    };
    Some(PointeeDoubleIndexedPath {
        descriptor_offset,
        outer_region,
        outer_offset,
        outer_byte_size,
        outer_stride,
        inner_region,
        inner_offset,
        inner_byte_size,
        inner_stride,
        field_offset,
    })
}

/// `Const*, SI, Const*, SI, Const*` with NO deref -- the inline 2D-array
/// element path. The mid-const between the indices folds into
/// `field_offset` (the address is a pure sum, so the adds commute).
fn direct_double_indexed_path(place: &omega_target_operations::Place) -> Option<DoubleIndexedPath> {
    let mut base_offset = 0usize;
    let mut indices: Vec<(
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
    )> = Vec::new();
    let mut trailing = 0usize;
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                if indices.is_empty() {
                    base_offset += offset;
                } else {
                    trailing += offset;
                }
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } => {
                if indices.len() == 2 {
                    return None;
                }
                indices.push((
                    *index_region,
                    *index_offset,
                    *index_byte_size,
                    *element_byte_size,
                ));
            }
            omega_target_operations::PlaceStep::Deref => return None,
        }
    }
    let [
        (outer_region, outer_offset, outer_byte_size, outer_stride),
        (inner_region, inner_offset, inner_byte_size, inner_stride),
    ] = indices[..]
    else {
        return None;
    };
    Some(DoubleIndexedPath {
        base_offset,
        outer_region,
        outer_offset,
        outer_byte_size,
        outer_stride,
        inner_region,
        inner_offset,
        inner_byte_size,
        inner_stride,
        field_offset: trailing,
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
        index_byte_size,
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
        index_byte_size: *index_byte_size,
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

pub fn x86_64_encode_write_place_integer_with_sites(
    target: &omega_target_operations::Place,
    value: i64,
    byte_size: usize,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_integer_write(target, value, byte_size)
}

#[allow(clippy::too_many_arguments)]
pub fn x86_64_encode_write_place_binary_with_sites(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target: &omega_target_operations::Place,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
    is_float: bool,
    domain: psi_numerics::arithmetic::ArithmeticDomain,
    target_signed: bool,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_binary_write(
        runtime_value_operands,
        target,
        byte_size,
        left,
        operator,
        right,
        is_float,
        domain,
        target_signed,
    )
}

/// The walker's operand-relocation anchor for a place binary write.
pub fn place_binary_operand_start_width(target: &omega_target_operations::Place) -> usize {
    x86_64::place_binary_operand_start_width(target)
}

/// The x86 prefix's deterministic cross-region index base positions.
pub fn place_binary_index_base_positions(
    target: &omega_target_operations::Place,
) -> Vec<(usize, omega_target_operations::RuntimeStorageRegion)> {
    x86_64::place_binary_index_base_positions(target).collect()
}

pub fn encode_runtime_machine_double_indexed_integer_write(
    architecture: Architecture,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_byte_size: usize,
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
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            byte_size,
            value,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_machine_double_indexed_integer_write(
            base_byte_offset,
            outer_index_offset,
            outer_index_region,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_region,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
            byte_size,
            value,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target_operations::{Place, PlaceStep, RuntimeStorageRegion, RuntimeValueOperand};

    fn storage_source() -> (
        psi_arena::Arena<RuntimeValueOperand>,
        RuntimeValueOperandHandle,
    ) {
        let mut operands = psi_arena::Arena::new();
        let source = operands.insert(RuntimeValueOperand::Storage {
            region: RuntimeStorageRegion::Machine,
            byte_offset: 96,
            byte_size: 4,
        });
        (operands, source)
    }

    fn encode_aarch64_convert(target: &Place) -> Result<Vec<u8>, Diagnostic> {
        let (operands, source) = storage_source();
        encode_write_place_convert(
            Architecture::Aarch64,
            &operands,
            target,
            8,
            source,
            4,
            false,
            false,
            false,
            false,
            false,
            false,
        )
    }

    fn pointee_double_indexed_place() -> Place {
        Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::Deref)
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 24,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 32,
                    index_byte_size: 8,
                    element_byte_size: 2,
                })
            })
            .expect("pointee double-indexed place")
    }

    #[test]
    fn pointee_double_indexed_read_write_shapes_retain_exact_geometry_and_depth_fences() {
        let source = pointee_double_indexed_place();
        let target = Place::at(RuntimeStorageRegion::Machine, 40);

        assert_eq!(
            classify_write_place_shape(&source),
            WritePlaceShape::PointeeDoubleIndexed {
                descriptor_offset: 0,
                outer_index_region: RuntimeStorageRegion::Machine,
                outer_index_offset: 24,
                outer_index_byte_size: 8,
                outer_stride: 8,
                inner_index_region: RuntimeStorageRegion::Machine,
                inner_index_offset: 32,
                inner_index_byte_size: 8,
                inner_stride: 2,
                field_byte_offset: 4,
            }
        );
        assert_eq!(
            classify_copy_places_shape(&source, &target),
            CopyPlacesShape::FromPointeeDoubleIndexed {
                descriptor_offset: 0,
                outer_index_region: RuntimeStorageRegion::Machine,
                outer_index_offset: 24,
                outer_index_byte_size: 8,
                outer_stride: 8,
                inner_index_region: RuntimeStorageRegion::Machine,
                inner_index_offset: 32,
                inner_index_byte_size: 8,
                inner_stride: 2,
                field_byte_offset: 4,
                target_offset: 40,
            }
        );

        let write = encode_write_place_integer(Architecture::Aarch64, &source, 17, 2)
            .expect("encode pointee double-indexed write");
        assert_eq!(
            write_place_integer_width(Architecture::Aarch64, &source, 17, 2)
                .expect("measure pointee double-indexed write"),
            write.len()
        );
        assert!(
            !encode_copy_places(Architecture::Aarch64, &source, &target, 2)
                .expect("encode pointee double-indexed read")
                .is_empty()
        );

        let too_deep = source
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 48,
                index_byte_size: 8,
                element_byte_size: 1,
            })
            .expect("six-step failure-fence place");
        assert_eq!(
            classify_write_place_shape(&too_deep),
            WritePlaceShape::Unsupported
        );
        assert!(!matches!(
            classify_copy_places_shape(&too_deep, &target),
            CopyPlacesShape::FromPointeeDoubleIndexed { .. }
        ));

        let second_deref = Place::at(RuntimeStorageRegion::RuntimeFrame, 0)
            .with_step(PlaceStep::Deref)
            .and_then(|place| place.with_step(PlaceStep::Deref))
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 24,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 32,
                    index_byte_size: 8,
                    element_byte_size: 2,
                })
            })
            .expect("double-deref failure-fence place");
        assert_eq!(
            classify_write_place_shape(&second_deref),
            WritePlaceShape::Unsupported
        );
    }

    #[test]
    fn aarch64_place_convert_serves_every_classified_place_shape() {
        let direct = Place::at(RuntimeStorageRegion::Machine, 16);
        let pointee = Place::at(RuntimeStorageRegion::RuntimeFrame, 24)
            .with_step(PlaceStep::Deref)
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(8)))
            .expect("pointee place");
        let frame_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::Deref)
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 40,
                    index_byte_size: 8,
                    element_byte_size: 16,
                })
            })
            .expect("frame-indexed place");
        let frame_base_indexed = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 56,
                index_byte_size: 8,
                element_byte_size: 16,
            })
            .expect("frame-base-indexed place");
        let machine_indexed = Place::at(RuntimeStorageRegion::Machine, 64)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 16,
            })
            .expect("machine-indexed place");
        let machine_double_indexed = machine_indexed
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 80,
                index_byte_size: 8,
                element_byte_size: 4,
            })
            .expect("machine-double-indexed place");

        for target in [
            direct,
            pointee,
            frame_indexed,
            frame_base_indexed,
            machine_indexed,
            machine_double_indexed,
        ] {
            let bytes = encode_aarch64_convert(&target)
                .unwrap_or_else(|diagnostic| panic!("{target:?}: {diagnostic:?}"));
            assert!(!bytes.is_empty(), "{target:?} must emit conversion bytes");
        }
    }

    #[test]
    fn all_frame_double_indexed_shape_is_opted_into_per_write_family() {
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 72,
                    index_byte_size: 8,
                    element_byte_size: 8,
                })
            })
            .expect("all-frame double-indexed target");

        assert_eq!(
            classify_write_place_shape(&target),
            WritePlaceShape::Unsupported
        );
        let shape = classify_frame_base_double_indexed_binary_shape(&target)
            .expect("binary classifier must retain the all-frame target");
        assert_eq!(shape.base_byte_offset, 32);
        assert_eq!(shape.outer_index_offset, 64);
        assert_eq!(shape.inner_index_offset, 72);
        assert_eq!(
            classify_frame_base_double_indexed_integer_shape(&target),
            Some(shape),
            "integer writes opt into the same closed address shape"
        );
        assert_eq!(
            classify_frame_base_double_indexed_convert_shape(&target),
            Some(shape),
            "conversion writes opt into the same closed address shape"
        );
    }

    #[test]
    fn all_frame_double_indexed_copy_can_target_a_frame_held_pointee() {
        let source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 112,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .expect("all-frame double-indexed source");
        let target = Place::at(RuntimeStorageRegion::RuntimeFrame, 120)
            .with_step(PlaceStep::Deref)
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(8)))
            .expect("frame-held pointee target");

        assert!(matches!(
            classify_copy_places_shape(&source, &target),
            CopyPlacesShape::FrameBaseDoubleIndexedToPointee {
                base_byte_offset: 32,
                outer_index_offset: 104,
                inner_index_offset: 112,
                source_field_byte_offset: 4,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        let bytes = encode_copy_places(Architecture::Aarch64, &source, &target, 12)
            .expect("encode the retained pointee copy shape");
        assert_eq!(
            bytes.len(),
            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_width(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                120, 8, 12,
            )
        );

        assert!(matches!(
            classify_copy_places_shape(&target, &source),
            CopyPlacesShape::PointeeToFrameBaseDoubleIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 32,
                outer_index_offset: 104,
                inner_index_offset: 112,
                target_field_byte_offset: 4,
                ..
            }
        ));
        let reverse = encode_copy_places(Architecture::Aarch64, &target, &source, 12)
            .expect("encode the retained reverse pointee copy shape");
        assert_eq!(
            reverse.len(),
            omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_width(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                120, 8, 12,
            )
        );

        let mixed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 44)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 88,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 96,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index frame double source");
        assert!(matches!(
            classify_copy_places_shape(&mixed_source, &target),
            CopyPlacesShape::FrameBaseDoubleIndexedToPointee {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert!(matches!(
            classify_copy_places_shape(&target, &mixed_source),
            CopyPlacesShape::PointeeToFrameBaseDoubleIndexed {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));

        let direct = Place::at(RuntimeStorageRegion::RuntimeFrame, 168);
        assert!(matches!(
            classify_copy_places_shape(&mixed_source, &direct),
            CopyPlacesShape::FromFrameBaseDoubleIndexed {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        let direct_read = encode_copy_places(Architecture::Aarch64, &mixed_source, &direct, 12)
            .expect("encode mixed-index frame-double direct read");
        assert_eq!(
            direct_read.len(),
            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
                RuntimeStorageRegion::Machine,
                RuntimeStorageRegion::RuntimeFrame,
                168,
                12,
            )
        );
        assert!(matches!(
            classify_copy_places_shape(&direct, &mixed_source),
            CopyPlacesShape::ToFrameBaseDoubleIndexed {
                outer_index_region: RuntimeStorageRegion::Machine,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        let direct_write = encode_copy_places(Architecture::Aarch64, &direct, &mixed_source, 12)
            .expect("encode mixed-index frame-double direct write");
        assert_eq!(
            direct_write.len(),
            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage_width(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::Machine,
                RuntimeStorageRegion::RuntimeFrame,
                168,
                12,
            )
        );

        let single_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 40)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 96,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .expect("all-frame indexed source");
        assert!(matches!(
            classify_copy_places_shape(&single_source, &target),
            CopyPlacesShape::FrameBaseIndexedToPointee {
                base_byte_offset: 40,
                index_offset: 96,
                source_field_byte_offset: 4,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        assert!(matches!(
            classify_copy_places_shape(&target, &single_source),
            CopyPlacesShape::PointeeToFrameBaseIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 40,
                index_offset: 96,
                target_field_byte_offset: 4,
                ..
            }
        ));
        assert_eq!(
            encode_copy_places(Architecture::Aarch64, &single_source, &target, 12)
                .expect("encode all-frame indexed pointee copy")
                .len(),
            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_width(RuntimeStorageRegion::RuntimeFrame, 120, 8, 12)
        );
        assert_eq!(
            encode_copy_places(Architecture::Aarch64, &target, &single_source, 12)
                .expect("encode reverse all-frame indexed pointee copy")
                .len(),
            omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_width(RuntimeStorageRegion::RuntimeFrame, 120, 8, 12)
        );
        let cross_single_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 48)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 88,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("machine-indexed frame source");
        assert!(matches!(
            classify_copy_places_shape(&cross_single_source, &target),
            CopyPlacesShape::FrameBaseIndexedToPointee {
                index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert!(matches!(
            classify_copy_places_shape(&target, &cross_single_source),
            CopyPlacesShape::PointeeToFrameBaseIndexed {
                index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));

        let double_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 160)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 128,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 136,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .and_then(|place| place.with_step(PlaceStep::ConstOffset(4)))
            .expect("all-frame double-indexed target");
        assert!(matches!(
            classify_copy_places_shape(&source, &double_target),
            CopyPlacesShape::FrameBaseDoubleIndexedPair {
                source_base_byte_offset: 32,
                source_outer_index_offset: 104,
                source_inner_index_offset: 112,
                target_base_byte_offset: 160,
                target_outer_index_offset: 128,
                target_inner_index_offset: 136,
                ..
            }
        ));
        let double_pair = encode_copy_places(Architecture::Aarch64, &source, &double_target, 12)
            .expect("encode retained all-frame double-indexed pair");
        assert_eq!(
            double_pair.len(),
            omega_isa_aarch64::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_width(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                12,
            )
        );

        let mixed_double_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 176)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 152,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index frame double target");
        assert!(matches!(
            classify_copy_places_shape(&mixed_source, &mixed_double_target),
            CopyPlacesShape::FrameBaseDoubleIndexedPair {
                source_outer_index_region: RuntimeStorageRegion::Machine,
                source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_inner_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert!(
            !encode_copy_places(
                Architecture::Aarch64,
                &mixed_source,
                &mixed_double_target,
                12,
            )
            .expect("encode retained mixed-index frame double pair")
            .is_empty()
        );

        let machine_source = Place::at(RuntimeStorageRegion::Machine, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::RuntimeFrame,
                    index_offset: 112,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index machine double source");
        assert!(matches!(
            classify_copy_places_shape(&machine_source, &target),
            CopyPlacesShape::MachineDoubleIndexedToPointee {
                base_byte_offset: 32,
                outer_index_region: RuntimeStorageRegion::Machine,
                outer_index_offset: 104,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                inner_index_offset: 112,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        let machine_to_pointee =
            encode_copy_places(Architecture::Aarch64, &machine_source, &target, 12)
                .expect("encode retained machine double-indexed pointee copy");
        assert_eq!(
            machine_to_pointee.len(),
            omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_runtime_pointee_width(
                120, 8, 12,
            )
        );
        assert!(matches!(
            classify_copy_places_shape(&target, &machine_source),
            CopyPlacesShape::PointeeToMachineDoubleIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 32,
                outer_index_region: RuntimeStorageRegion::Machine,
                outer_index_offset: 104,
                inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                inner_index_offset: 112,
                ..
            }
        ));
        let pointee_to_machine =
            encode_copy_places(Architecture::Aarch64, &target, &machine_source, 12)
                .expect("encode retained reverse machine double-indexed pointee copy");
        assert_eq!(
            pointee_to_machine.len(),
            omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_double_indexed_width(
                120, 8, 12,
            )
        );
        let machine_indexed_source = Place::at(RuntimeStorageRegion::Machine, 200)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("frame-indexed machine source");
        assert!(matches!(
            classify_copy_places_shape(&machine_indexed_source, &target),
            CopyPlacesShape::MachineIndexedToPointee {
                base_byte_offset: 200,
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                pointer_byte_offset: 120,
                target_field_byte_offset: 8,
                ..
            }
        ));
        assert_eq!(
            encode_copy_places(Architecture::Aarch64, &machine_indexed_source, &target, 12)
                .expect("encode retained machine indexed pointee copy")
                .len(),
            omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_runtime_pointee_width(
                120, 8, 12,
            )
        );
        assert!(matches!(
            classify_copy_places_shape(&target, &machine_indexed_source),
            CopyPlacesShape::PointeeToMachineIndexed {
                pointer_byte_offset: 120,
                source_field_byte_offset: 8,
                base_byte_offset: 200,
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 144,
                ..
            }
        ));
        assert_eq!(
            encode_copy_places(Architecture::Aarch64, &target, &machine_indexed_source, 12)
                .expect("encode retained reverse machine indexed pointee copy")
                .len(),
            omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_indexed_width(
                120, 8, 12,
            )
        );
        let machine_target = Place::at(RuntimeStorageRegion::Machine, 160)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 128,
                index_byte_size: 8,
                element_byte_size: 36,
            })
            .and_then(|place| {
                place.with_step(PlaceStep::ScaledIndex {
                    index_region: RuntimeStorageRegion::Machine,
                    index_offset: 136,
                    index_byte_size: 8,
                    element_byte_size: 12,
                })
            })
            .expect("mixed-index machine double target");
        assert!(matches!(
            classify_copy_places_shape(&machine_source, &machine_target),
            CopyPlacesShape::MachineDoubleIndexedPair {
                source_outer_index_region: RuntimeStorageRegion::Machine,
                source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_inner_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        let machine_double_pair =
            encode_copy_places(Architecture::Aarch64, &machine_source, &machine_target, 12)
                .expect("encode retained machine double-indexed pair");
        assert_eq!(
            machine_double_pair.len(),
            omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_width(
                RuntimeStorageRegion::Machine,
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::Machine,
                12,
            )
        );
        assert!(matches!(
            classify_copy_places_shape(&machine_source, &mixed_double_target),
            CopyPlacesShape::CrossRegionDoubleIndexedPair {
                source_outer_index_region: RuntimeStorageRegion::Machine,
                source_inner_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_outer_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_inner_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        let cross_region_double_pair = encode_copy_places(
            Architecture::Aarch64,
            &machine_source,
            &mixed_double_target,
            12,
        )
        .expect("encode retained cross-region double-indexed pair");
        assert_eq!(
            cross_region_double_pair.len(),
            omega_isa_aarch64::runtime_storage_copy_cross_region_double_indexed_pair_width(12)
        );

        let indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("all-frame indexed source");
        let indexed_target = Place::at(RuntimeStorageRegion::RuntimeFrame, 160)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 112,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("all-frame indexed target");
        assert!(matches!(
            classify_copy_places_shape(&indexed_source, &indexed_target),
            CopyPlacesShape::FrameBaseIndexedPair {
                source_base_byte_offset: 32,
                source_index_region: RuntimeStorageRegion::RuntimeFrame,
                source_index_offset: 104,
                target_base_byte_offset: 160,
                target_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_index_offset: 112,
                ..
            }
        ));
        let indexed_pair =
            encode_copy_places(Architecture::Aarch64, &indexed_source, &indexed_target, 12)
                .expect("encode retained all-frame indexed pair");
        assert_eq!(
            indexed_pair.len(),
            omega_isa_aarch64::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_width(
                RuntimeStorageRegion::RuntimeFrame,
                RuntimeStorageRegion::RuntimeFrame,
                12,
            )
        );

        let mixed_indexed_source = Place::at(RuntimeStorageRegion::RuntimeFrame, 32)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::Machine,
                index_offset: 104,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("mixed-index frame source");
        assert!(matches!(
            classify_copy_places_shape(&mixed_indexed_source, &indexed_target),
            CopyPlacesShape::FrameBaseIndexedPair {
                source_index_region: RuntimeStorageRegion::Machine,
                target_index_region: RuntimeStorageRegion::RuntimeFrame,
                ..
            }
        ));
        assert!(
            !encode_copy_places(
                Architecture::Aarch64,
                &mixed_indexed_source,
                &indexed_target,
                12,
            )
            .expect("encode retained mixed-index frame pair")
            .is_empty()
        );

        let cross_region_source = Place::at(RuntimeStorageRegion::Machine, 200)
            .with_step(PlaceStep::ScaledIndex {
                index_region: RuntimeStorageRegion::RuntimeFrame,
                index_offset: 120,
                index_byte_size: 8,
                element_byte_size: 12,
            })
            .expect("cross-region indexed source");
        assert!(matches!(
            classify_copy_places_shape(&cross_region_source, &mixed_indexed_source),
            CopyPlacesShape::CrossRegionIndexedPair {
                source_index_region: RuntimeStorageRegion::RuntimeFrame,
                target_index_region: RuntimeStorageRegion::Machine,
                ..
            }
        ));
        assert!(
            !encode_copy_places(
                Architecture::Aarch64,
                &cross_region_source,
                &mixed_indexed_source,
                12,
            )
            .expect("encode retained cross-region indexed pair")
            .is_empty()
        );
    }
}
