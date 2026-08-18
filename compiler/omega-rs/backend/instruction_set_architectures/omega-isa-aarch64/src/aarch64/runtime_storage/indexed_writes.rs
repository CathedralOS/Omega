use omega_calling_conventions::{MachineRegister, RegisterSet};
use omega_target_operations::{
    RuntimeValueOperandHandle, RuntimeValueOperandSource, StateGuardOperator,
};
use psi_diagnostics::Diagnostic;

use super::{
    RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS, RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
    append_double_index_address_math, append_double_index_bases,
    append_fixed_width_load_x_from_x_offset, append_runtime_binary_operation,
    append_runtime_frame_base_index_target_address_with_index_region,
    append_runtime_frame_index_target_address_with_index_region,
    append_runtime_machine_index_target_address, append_runtime_storage_result_write,
    append_runtime_value_operand, runtime_binary_operation_byte_size,
    runtime_storage_copy_from_runtime_machine_double_indexed_clobbers,
};
use crate::aarch64::primitives::{
    append_unsigned_immediate, append_unsigned_immediate_padded,
    append_unsigned_immediate_w_padded, encode_add_page_offset_placeholder,
    encode_adrp_placeholder, encode_store_w_to_x, encode_store_w17_to_x16, encode_store_x_to_x,
    encode_store_x17_to_x16,
};
use crate::aarch64::widths::{
    runtime_frame_base_indexed_integer_write_with_index_region_width,
    runtime_frame_indexed_binary_write_width, runtime_frame_indexed_integer_write_width,
    runtime_machine_indexed_integer_write_width,
};

pub fn encode_runtime_frame_indexed_integer_write(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_indexed_integer_write_with_index_region(
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    )
}

pub fn encode_runtime_frame_indexed_integer_write_with_index_region(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_integer_write_width(
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_frame_index_target_address_with_index_region(
        &mut bytes,
        16,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
    )?;
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }

    Ok(bytes)
}

/// Exact scratch footprint of an immediate integer write through a
/// frame-held indexed descriptor. The fixed-width address recipe always
/// writes x16/x17/x19/x20/x21/x26; a machine-resident index additionally
/// materializes its base in x15.
pub fn runtime_frame_indexed_integer_write_clobbers(
    index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(21),
        MachineRegister::Aarch64X(26),
    ];
    if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Write an immediate integer through a frame-held pointer followed by two
/// independent runtime indices. The frame root supplies the pointer slot and
/// any frame-held indices; one optional machine root supplies either or both
/// machine-held indices. The plan-laid outer stride and compiler-derived inner
/// stride remain separate operands all the way to address materialization.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_pointee_double_indexed_integer_write(
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
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte pointee-double-indexed values yet"
        )));
    }
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::new();
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_fixed_width_load_x_from_x_offset(&mut bytes, 16, 20, descriptor_offset, 19);
    if outer_index_region != frame || inner_index_region != frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == frame { 20 } else { 15 },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == frame { 20 } else { 15 },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
    )?;
    append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
    match byte_size {
        8 => bytes.extend(encode_store_x_to_x(17, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?),
    }
    Ok(bytes)
}

pub fn runtime_pointee_double_indexed_integer_write_clobbers(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ];
    if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

pub fn encode_runtime_frame_base_indexed_integer_write(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_base_indexed_integer_write_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_integer_write_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_frame_base_indexed_integer_write_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        ),
    );
    append_runtime_frame_base_index_target_address_with_index_region(
        &mut bytes,
        16,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
    )?;
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w17_to_x16(0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x17_to_x16(0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime indexed integers yet"
            )));
        }
    }
    Ok(bytes)
}

/// Exact scratch footprint of an immediate integer write into an inline
/// runtime-frame array. x20 owns the frame base, x16 the element address,
/// x17 the index/value, x26 the scaled index, and the shared scale helper
/// writes x19.
pub fn runtime_frame_base_indexed_integer_write_clobbers() -> RegisterSet {
    runtime_frame_base_indexed_integer_write_with_index_region_clobbers(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
    )
}

pub fn runtime_frame_base_indexed_integer_write_with_index_region_clobbers(
    index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ];
    if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

pub fn encode_runtime_machine_indexed_integer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_indexed_integer_write_width(
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
    ));
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )?;
    match byte_size {
        1 | 2 | 4 => {
            append_unsigned_immediate_w_padded(&mut bytes, 17, value as u32);
            bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?);
        }
        8 => {
            append_unsigned_immediate_padded(&mut bytes, 17, value as u64);
            bytes.extend(encode_store_x_to_x(17, 16, 0)?);
        }
        _ => {
            return Err(Diagnostic::error(format!(
                "AArch64 MVP encoder cannot store {byte_size}-byte runtime integers yet"
            )));
        }
    }

    Ok(bytes)
}

/// Exact scratch footprint of an immediate integer write into an inline
/// machine array. x16 owns the element address, x20 the machine/frame index
/// base, x17 the index/value, x26 the scaled index, and offset/scale helpers
/// write x19.
pub fn runtime_machine_indexed_integer_write_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

pub fn encode_runtime_frame_indexed_binary_write(
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
    encode_runtime_frame_indexed_binary_write_with_index_region(
        runtime_value_operands,
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_binary_write_with_index_region(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    descriptor_offset: usize,
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
    let expected_width =
        runtime_frame_indexed_binary_write_width(
            runtime_value_operands,
            element_byte_size,
            field_byte_offset,
            byte_size,
            left,
            operator,
            right,
        ) + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8;
    let mut bytes = Vec::with_capacity(expected_width);
    append_runtime_frame_index_target_address_with_index_region(
        &mut bytes,
        16,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        left,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        right,
    )?;
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_binary_write(
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
    encode_runtime_frame_base_indexed_binary_write_with_index_region(
        runtime_value_operands,
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left,
        operator,
        right,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_binary_write_with_index_region(
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
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_frame_base_indexed_binary_write_with_index_region_width(
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
    );
    append_runtime_frame_base_index_target_address_with_index_region(
        &mut bytes,
        16,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        17,
        26,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        left,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        right,
    )?;
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

/// RMW into a machine-resident indexed element (`self.tallies[k] += 1`): the
/// machine-index address helper walks x16 to the element (its optional
/// frame-index pair sits at the constant the string-write offset helper
/// exposes), the operands evaluate into x17/x26 (preserving x16), and the
/// result stores at [x16, 0] -- the machine-region mirror of the working
/// frame-base flavor above.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_binary_write(
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
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_indexed_binary_write_width(
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
    );
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        left,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        right,
    )?;
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

/// Write twin: `grid[i][j] = <literal>` -- the same address math, then the
/// value immediate materialized into x17 (AFTER every relocation, so its
/// variable width perturbs no reloc offset) and stored at the element.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_integer_write(
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
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte double-indexed values yet"
        )));
    }
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            inner_index_region,
            value,
        ),
    );
    let (outer_base, inner_base) =
        append_double_index_bases(&mut bytes, outer_index_region, inner_index_region);
    append_double_index_address_math(
        &mut bytes,
        outer_base,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_base,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    append_unsigned_immediate(&mut bytes, 17, value as u64);
    match byte_size {
        8 => bytes.extend(encode_store_x_to_x(17, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?),
    }
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_double_indexed_integer_write_width(
            outer_index_region,
            inner_index_region,
            value,
        )
    );
    Ok(bytes)
}

/// Write a literal into an all-frame double-indexed element. The collection
/// and both index slots share the one relocated frame base in x16.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_integer_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_size, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_size}-byte frame-double-indexed values yet"
        )));
    }
    let expected_width =
        crate::aarch64::widths::runtime_frame_base_double_indexed_integer_write_width(value);
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_double_index_address_math(
        &mut bytes,
        16,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        16,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    append_unsigned_immediate(&mut bytes, 17, value as u64);
    match byte_size {
        8 => bytes.extend(encode_store_x_to_x(17, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, 0, byte_size)?),
    }
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_frame_base_double_indexed_integer_write_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(26),
    ])
}

/// Exact scratch footprint of a double-indexed immediate write. The fixed
/// address program writes x14/x16/x17/x26; x15 materializes the one shared
/// frame base exactly when either index is frame-resident.
pub fn runtime_machine_double_indexed_integer_write_clobbers(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    runtime_storage_copy_from_runtime_machine_double_indexed_clobbers(
        outer_index_region,
        inner_index_region,
    )
}

/// RMW into a double-indexed element (`grid[i][j] += 1`): the double-index
/// bases + math walk x16 to the element, the operands evaluate into x17/x26
/// (preserving x16), and the result stores at [x16, 0].
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_binary_write(
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
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_double_indexed_binary_write_width(
            runtime_value_operands,
            outer_index_region,
            inner_index_region,
            byte_size,
            left,
            operator,
            right,
        ),
    );
    let (outer_base, inner_base) =
        append_double_index_bases(&mut bytes, outer_index_region, inner_index_region);
    append_double_index_address_math(
        &mut bytes,
        outer_base,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_base,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        left,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        right,
    )?;
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    Ok(bytes)
}

/// Write a binary result into an all-frame double-indexed element. The
/// collection and both index slots share the one relocated frame base in x16.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_binary_write(
    runtime_value_operands: &impl RuntimeValueOperandSource,
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperandHandle,
    operator: StateGuardOperator,
    right: RuntimeValueOperandHandle,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_frame_base_double_indexed_binary_write_width(
            runtime_value_operands,
            byte_size,
            left,
            operator,
            right,
        );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_double_index_address_math(
        &mut bytes,
        16,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        16,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        17,
        RUNTIME_VALUE_LEFT_SCRATCH_REGISTERS,
        left,
    )?;
    append_runtime_value_operand(
        runtime_value_operands,
        &mut bytes,
        26,
        RUNTIME_VALUE_RIGHT_SCRATCH_REGISTERS,
        right,
    )?;
    append_runtime_binary_operation(
        &mut bytes,
        17,
        operator,
        26,
        runtime_binary_operation_byte_size(
            runtime_value_operands,
            operator,
            left,
            right,
            byte_size,
        ),
    )?;
    append_runtime_storage_result_write(&mut bytes, 0, byte_size)?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}
