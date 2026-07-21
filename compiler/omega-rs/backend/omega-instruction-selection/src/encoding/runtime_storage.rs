use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};

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

pub fn encode_atomic_load_to_storage(
    architecture: Architecture,
    source_offset: usize,
    byte_size: usize,
    result_offset: usize,
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::Load(ordering) = ordering else {
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
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::Store(ordering) = ordering else {
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
            ordering == omega_core::atomic::MemoryOrdering::SeqCst,
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
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
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
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
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
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::ReadModifyWrite(ordering) = ordering else {
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

pub fn encode_atomic_swap(
    architecture: Architecture,
    runtime_value_operands: &impl RuntimeValueOperandSource,
    target_offset: usize,
    byte_size: usize,
    result_offset: usize,
    new_value: RuntimeValueOperandHandle,
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::Swap(ordering) = ordering else {
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
    ordering: omega_core::atomic::AtomicOrderingPlan,
) -> Result<Vec<u8>, Diagnostic> {
    let omega_core::atomic::AtomicOrderingPlan::CompareExchange { success, .. } = ordering else {
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
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
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
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_stride: double.inner_stride,
                field_byte_offset: double.field_offset,
            };
        }
        return WritePlaceShape::Unsupported;
    }
    if let Some(indexed) = direct_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine {
            return WritePlaceShape::MachineIndexed {
                base_byte_offset: indexed.pointer_offset,
                index_region: indexed.index_region,
                index_offset: indexed.index_offset,
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
                element_byte_size: indexed.element_byte_size,
                field_byte_offset: indexed.field_offset,
            };
        }
        return WritePlaceShape::Unsupported;
    }
    if let Some(indexed) = single_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            && indexed.index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        {
            return WritePlaceShape::FrameIndexed {
                descriptor_offset: indexed.pointer_offset,
                index_offset: indexed.index_offset,
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

/// Write rung 2a: the place-shaped integer write. x86_64 rides the
/// materializer; aarch64 REFUSES LOUDLY until its decompose rung (zero
/// producers exist yet -- the old Write*Integer variants still carry the
/// corpus there).
pub fn encode_write_place_integer(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    value: i64,
    byte_size: usize,
) -> Result<Vec<u8>, Diagnostic> {
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
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_integer_write(
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_integer_write(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_integer_write(
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            ),
            WritePlaceShape::MachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_double_indexed_integer_write(
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
            WritePlaceShape::Unsupported => Err(Diagnostic::error(
                "WritePlaceInteger on aarch64 serves direct, pointee, frame-indexed, \
                 frame-base-indexed, machine-indexed, and machine-double-indexed \
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
)> {
    if place.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return None;
    }
    single_indexed_path(place).map(|indexed| {
        (
            indexed.pointer_offset,
            indexed.index_region,
            indexed.index_offset,
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
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            WritePlaceShape::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_address_to_runtime_frame_write(
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
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
                    element_byte_size,
                    field_byte_offset,
                )) = place_frame_deref_indexed_path(source)
                {
                    return aarch64::encode_runtime_frame_indexed_address_to_runtime_frame_write(
                        index_region,
                        descriptor_offset,
                        index_offset,
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
/// string encoders (which serve direct/pointee/frame-indexed and the
/// frame-resident machine-indexed shapes only -- everything else refuses
/// loudly until the aarch64 place materializer lands).
pub fn encode_write_place_string(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
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
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_frame_indexed_string_write(
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_length,
            ),
            WritePlaceShape::MachineIndexed {
                base_byte_offset,
                index_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => aarch64::encode_runtime_machine_indexed_string_write(
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_length,
            ),
            _ => Err(Diagnostic::error(
                "WritePlaceString on aarch64 serves direct, pointee, frame-indexed, \
                 and frame-resident machine-indexed place shapes only until the \
                 aarch64 place materializer lands; this shape refuses loudly",
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
/// the materializer; aarch64 decomposes to the retained carrier encoders
/// (direct + pointee, the only shapes the retired kinds spelled).
pub fn encode_write_place_bounded_buffer(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
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
            _ => Err(Diagnostic::error(
                "WritePlaceBoundedBuffer on aarch64 serves direct and pointee place \
                 shapes only until the aarch64 place materializer lands; this shape \
                 refuses loudly",
            )),
        },
    }
}

/// One source of truth: the encoder's output length.
pub fn write_place_bounded_buffer_width(
    architecture: Architecture,
    target: &omega_target_operations::Place,
    literal: &str,
) -> Result<usize, Diagnostic> {
    encode_write_place_bounded_buffer(architecture, target, literal).map(|bytes| bytes.len())
}

pub fn x86_64_encode_write_place_bounded_buffer_with_sites(
    target: &omega_target_operations::Place,
    literal: &str,
) -> Result<(Vec<u8>, omega_isa_x86_64::PlaceCopySites), Diagnostic> {
    x86_64::encode_place_bounded_buffer_write(target, literal)
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
    domain: omega_core::arithmetic::ArithmeticDomain,
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
            if !matches!(shape, WritePlaceShape::Direct { .. })
                && (is_float || domain != omega_core::arithmetic::ArithmeticDomain::Exact)
            {
                return Err(Diagnostic::error(
                    "WritePlaceBinary on aarch64: shaped (deref/indexed) targets \
                     serve Exact integer domains only until the aarch64 place \
                     materializer lands",
                ));
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
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_frame_indexed_binary_write(
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
                WritePlaceShape::FrameBaseIndexed {
                    base_byte_offset,
                    index_offset,
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_frame_base_indexed_binary_write(
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
                WritePlaceShape::MachineIndexed {
                    base_byte_offset,
                    index_region,
                    index_offset,
                    element_byte_size,
                    field_byte_offset,
                } => aarch64::encode_runtime_machine_indexed_binary_write(
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
                WritePlaceShape::MachineDoubleIndexed {
                    base_byte_offset,
                    outer_index_region,
                    outer_index_offset,
                    outer_stride,
                    inner_index_region,
                    inner_index_offset,
                    inner_stride,
                    field_byte_offset,
                } => aarch64::encode_runtime_machine_double_indexed_binary_write(
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
                WritePlaceShape::Unsupported => Err(Diagnostic::error(
                    "WritePlaceBinary on aarch64 serves direct, pointee, frame-indexed, \
                     frame-base-indexed, machine-indexed, and machine-double-indexed \
                     place shapes only until the aarch64 place materializer lands",
                )),
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
    domain: omega_core::arithmetic::ArithmeticDomain,
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
            CopyPlacesShape::FromMachineDoubleIndexed {
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_stride,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
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
            ),
            CopyPlacesShape::FromFrameBaseDoubleIndexed {
                base_byte_offset,
                outer_index_offset,
                outer_stride,
                inner_index_offset,
                inner_stride,
                field_byte_offset,
                target_offset,
            } => aarch64::encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
                base_byte_offset,
                outer_index_offset,
                outer_stride,
                inner_index_offset,
                inner_stride,
                field_byte_offset,
                target_offset,
                byte_count,
            ),
            CopyPlacesShape::ToMachineDoubleIndexed {
                source_offset,
                base_byte_offset,
                outer_index_region,
                outer_index_offset,
                outer_stride,
                inner_index_region,
                inner_index_offset,
                inner_stride,
                field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
                source.region,
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
            ),
            CopyPlacesShape::MachineIndexedPair {
                source_base_byte_offset,
                source_index_region,
                source_index_offset,
                source_element_byte_size,
                source_field_byte_offset,
                target_base_byte_offset,
                target_index_region,
                target_index_offset,
                target_element_byte_size,
                target_field_byte_offset,
            } => aarch64::encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
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
    /// A MACHINE inline 2D-array element read (`m[i][j]` -- no deref):
    /// the double-indexed copy. Index-slot regions vary per index.
    FromMachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_stride: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// A FRAME inline 2D-array element read into a frame slot: all-frame,
    /// two indices, no deref.
    FromFrameBaseDoubleIndexed {
        base_byte_offset: usize,
        outer_index_offset: usize,
        outer_stride: usize,
        inner_index_offset: usize,
        inner_stride: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    /// The machine inline 2D-array element WRITE (`m[i][j] = v` -- a
    /// const-offset source into a double-indexed machine target).
    ToMachineDoubleIndexed {
        source_offset: usize,
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    /// `arr[i] = arr[j]` on machine inline arrays: ONE runtime index per
    /// side, both sides machine-resident, no deref.
    MachineIndexedPair {
        source_base_byte_offset: usize,
        source_index_region: omega_target_operations::RuntimeStorageRegion,
        source_index_offset: usize,
        source_element_byte_size: usize,
        source_field_byte_offset: usize,
        target_base_byte_offset: usize,
        target_index_region: omega_target_operations::RuntimeStorageRegion,
        target_index_offset: usize,
        target_element_byte_size: usize,
        target_field_byte_offset: usize,
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
    // The DOUBLE-indexed inline 2D-array reads first (a double path is
    // never a single path -- the recognizers refuse each other's shapes).
    if let Some(double) = direct_double_indexed_path(source) {
        if let Some(target_offset) = target.const_offset() {
            if source.region == omega_target_operations::RuntimeStorageRegion::Machine {
                return CopyPlacesShape::FromMachineDoubleIndexed {
                    base_byte_offset: double.base_offset,
                    outer_index_region: double.outer_region,
                    outer_index_offset: double.outer_offset,
                    outer_stride: double.outer_stride,
                    inner_index_region: double.inner_region,
                    inner_index_offset: double.inner_offset,
                    inner_stride: double.inner_stride,
                    field_byte_offset: double.field_offset,
                    target_offset,
                };
            }
            // Any const-offset target serves: the retained encoder is
            // ..._to_runtime_storage and the walker patches the target
            // base by its own region.
            if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && double.outer_region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && double.inner_region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                return CopyPlacesShape::FromFrameBaseDoubleIndexed {
                    base_byte_offset: double.base_offset,
                    outer_index_offset: double.outer_offset,
                    outer_stride: double.outer_stride,
                    inner_index_offset: double.inner_offset,
                    inner_stride: double.inner_stride,
                    field_byte_offset: double.field_offset,
                    target_offset,
                };
            }
        }
        return CopyPlacesShape::General;
    }
    if let Some(double) = direct_double_indexed_path(target) {
        if target.region == omega_target_operations::RuntimeStorageRegion::Machine
            && let Some(source_offset) = source.const_offset()
        {
            return CopyPlacesShape::ToMachineDoubleIndexed {
                source_offset,
                base_byte_offset: double.base_offset,
                outer_index_region: double.outer_region,
                outer_index_offset: double.outer_offset,
                outer_stride: double.outer_stride,
                inner_index_region: double.inner_region,
                inner_index_offset: double.inner_offset,
                inner_stride: double.inner_stride,
                field_byte_offset: double.field_offset,
            };
        }
        return CopyPlacesShape::General;
    }
    if let Some(indexed) = direct_indexed_path(source) {
        // `arr[i] = arr[j]`: one runtime index EACH side, both machine.
        if let Some(target_indexed) = direct_indexed_path(target)
            && source.region == omega_target_operations::RuntimeStorageRegion::Machine
            && target.region == omega_target_operations::RuntimeStorageRegion::Machine
        {
            return CopyPlacesShape::MachineIndexedPair {
                source_base_byte_offset: indexed.pointer_offset,
                source_index_region: indexed.index_region,
                source_index_offset: indexed.index_offset,
                source_element_byte_size: indexed.element_byte_size,
                source_field_byte_offset: indexed.field_offset,
                target_base_byte_offset: target_indexed.pointer_offset,
                target_index_region: target_indexed.index_region,
                target_index_offset: target_indexed.index_offset,
                target_element_byte_size: target_indexed.element_byte_size,
                target_field_byte_offset: target_indexed.field_offset,
            };
        }
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

struct DoubleIndexedPath {
    base_offset: usize,
    outer_region: omega_target_operations::RuntimeStorageRegion,
    outer_offset: usize,
    outer_stride: usize,
    inner_region: omega_target_operations::RuntimeStorageRegion,
    inner_offset: usize,
    inner_stride: usize,
    field_offset: usize,
}

/// `Const*, SI, Const*, SI, Const*` with NO deref -- the inline 2D-array
/// element path. The mid-const between the indices folds into
/// `field_offset` (the address is a pure sum, so the adds commute).
fn direct_double_indexed_path(place: &omega_target_operations::Place) -> Option<DoubleIndexedPath> {
    let mut base_offset = 0usize;
    let mut indices: Vec<(omega_target_operations::RuntimeStorageRegion, usize, usize)> =
        Vec::new();
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
                element_byte_size,
            } => {
                if indices.len() == 2 {
                    return None;
                }
                indices.push((*index_region, *index_offset, *element_byte_size));
            }
            omega_target_operations::PlaceStep::Deref => return None,
        }
    }
    let [
        (outer_region, outer_offset, outer_stride),
        (inner_region, inner_offset, inner_stride),
    ] = indices[..]
    else {
        return None;
    };
    Some(DoubleIndexedPath {
        base_offset,
        outer_region,
        outer_offset,
        outer_stride,
        inner_region,
        inner_offset,
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
    domain: omega_core::arithmetic::ArithmeticDomain,
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
