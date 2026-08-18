use omega_calling_conventions::{MachineRegister, RegisterSet};
use psi_diagnostics::Diagnostic;

use super::{
    append_add_constant_to_x_register, append_double_index_address_math, append_double_index_bases,
    append_fixed_shape_index_element_address, append_fixed_width_load_x_from_x_offset,
    append_load_data_from_x_offset,
    append_runtime_frame_base_index_target_address_with_index_region,
    append_runtime_frame_index_target_address_with_index_region,
    append_runtime_machine_index_target_address, append_runtime_storage_load,
    append_single_index_address_math, append_store_data_to_x_offset, data_offset_encodable,
    runtime_pointee_double_indexed_integer_write_clobbers,
};
use crate::aarch64::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_load_w_from_x,
    encode_load_x_from_x, encode_move_x_register, encode_store_w_to_x, encode_store_x_to_x,
};
use crate::aarch64::widths::{
    runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width,
    runtime_storage_copy_to_runtime_pointee_width, runtime_storage_copy_width,
};

pub fn encode_runtime_storage_copy(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_width(
        source_offset,
        target_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    append_add_constant_to_x_register(&mut bytes, 17, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 26, 16, offset, chunk_size, 19)?;
        append_store_data_to_x_offset(&mut bytes, 26, 17, offset, chunk_size, 20)?;
        Ok(())
    })?;

    Ok(bytes)
}

/// Exact scratch footprint of the direct runtime-storage copy encoder above.
/// x16/x17 hold the two bases, x26 stages non-empty chunks, x19 materializes
/// large base/chunk offsets, and x20 is the target-side large chunk scratch.
pub fn runtime_storage_copy_clobbers(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)];
    if byte_count > 0 {
        registers.push(MachineRegister::Aarch64X(26));
    }
    if source_offset > 4095 || target_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        if !data_offset_encodable(offset, chunk_size) {
            registers.extend([MachineRegister::Aarch64X(19), MachineRegister::Aarch64X(20)]);
        }
        Ok(())
    })
    .expect("runtime copy chunk partition is total");
    RegisterSet::new(registers)
}

/// Copy a doubly runtime-indexed leaf below a frame-held pointer into direct
/// frame or machine storage. One frame root supplies the pointer and any
/// frame-held indices; one shared machine root supplies the direct machine
/// target and either/both machine-held indices.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_pointee_double_indexed_to_runtime_storage(
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
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::new();
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_fixed_width_load_x_from_x_offset(&mut bytes, 16, 20, descriptor_offset, 19);
    if target_region != frame || outer_index_region != frame || inner_index_region != frame {
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
    let target_base = if target_region == frame { 20 } else { 15 };
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(
            &mut bytes,
            17,
            target_base,
            target_offset + offset,
            chunk_size,
            19,
        )?;
        Ok(())
    })?;
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_pointee_double_indexed_clobbers(
    target_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = runtime_pointee_double_indexed_integer_write_clobbers(
        outer_index_region,
        inner_index_region,
    )
    .as_slice()
    .to_vec();
    if target_region == omega_target_operations::RuntimeStorageRegion::Machine {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

pub fn encode_runtime_storage_copy_to_runtime_frame_indexed(
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        source_offset,
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_count,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_to_runtime_frame_indexed_with_regions(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let needs_source_machine_base = source_region
        == omega_target_operations::RuntimeStorageRegion::Machine
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let expected_width = crate::aarch64::widths::runtime_storage_copy_to_runtime_frame_indexed_width(
        source_offset,
        element_byte_size,
        field_byte_offset,
        byte_count,
    ) + usize::from(
        index_region == omega_target_operations::RuntimeStorageRegion::Machine,
    ) * 8 + usize::from(needs_source_machine_base) * 8;
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
    let source_base = match source_region {
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame => 20,
        omega_target_operations::RuntimeStorageRegion::Machine
            if index_region == omega_target_operations::RuntimeStorageRegion::Machine =>
        {
            15
        }
        omega_target_operations::RuntimeStorageRegion::Machine => {
            bytes.extend(encode_adrp_placeholder(15));
            bytes.extend(encode_add_page_offset_placeholder(15));
            15
        }
    };
    append_add_constant_to_x_register(&mut bytes, source_base, source_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, source_base, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;

    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_storage_copy_from_runtime_frame_indexed_with_index_region(
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_with_index_region(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
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
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_with_index_region(
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
        byte_count,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_with_index_region(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_storage_width(
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
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
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

/// Exact scratch footprint shared by both runtime-frame-indexed source copy
/// encoders above. Address formation always writes x16/x17/x19/x20/x21/x26;
/// a machine-resident index additionally materializes its base in x15. Later
/// chunk and large-offset paths stay within that same closed set.
pub fn runtime_storage_copy_from_runtime_frame_indexed_clobbers() -> RegisterSet {
    runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
    )
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
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

/// Exact scratch footprint of the direct-frame-source to frame-indexed target
/// encoder. Its address formation and copy use the same closed set as the
/// indexed-source mirror.
pub fn runtime_storage_copy_to_runtime_frame_indexed_clobbers() -> RegisterSet {
    runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
    )
}

pub fn runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
    source_region: omega_target_operations::RuntimeStorageRegion,
    index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers =
        runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(index_region)
            .as_slice()
            .to_vec();
    if source_region == omega_target_operations::RuntimeStorageRegion::Machine {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Exact scratch footprint of the frame-indexed-source to frame-held-pointee
/// encoder. It forms the same indexed address, then reuses x20 for the target
/// pointee, without introducing another scratch register.
pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers() -> RegisterSet
{
    runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
    )
}

pub fn runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
    index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(index_region)
}

/// Exact scratch footprint of the frame-resident inline-array read. x16 walks
/// the source element, x24 preserves the unbiased frame base, x20/x17/x26 form
/// the indexed address, and x17 also stages each copied chunk.
pub fn runtime_storage_copy_from_runtime_frame_base_indexed_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ])
}

/// Write a direct storage slot into a frame-resident inline array. The target,
/// index, and any frame source share x20's leading frame base; a machine source
/// receives a distinct x15 pair after the target address is formed.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 2 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_count}-byte frame-indexed values yet"
        )));
    }
    let expected_width = crate::aarch64::widths::runtime_storage_copy_to_runtime_frame_base_indexed_from_runtime_storage_width(
        source_region,
        source_offset,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
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
    let source_base =
        if source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            20
        } else if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            bytes.extend(encode_adrp_placeholder(15));
            bytes.extend(encode_add_page_offset_placeholder(15));
            15
        };
    append_load_data_from_x_offset(&mut bytes, 24, source_base, source_offset, byte_count, 19)?;
    match byte_count {
        8 => bytes.extend(encode_store_x_to_x(24, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(24, 16, 0, byte_count)?),
    }
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_to_runtime_frame_base_indexed_clobbers(
    source_region: omega_target_operations::RuntimeStorageRegion,
    index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ];
    if source_region == omega_target_operations::RuntimeStorageRegion::Machine
        || index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Offset of the second page-pair in the to-runtime-storage variant. A
/// frame-to-frame copy reuses the opening frame base and has no second site.
pub fn runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    crate::aarch64::widths::runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_storage_width(
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_fixed_indexed_to_runtime_pointee_width(
            element_index,
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    let source_offset = element_index
        .checked_mul(element_byte_size)
        .and_then(|offset| offset.checked_add(source_field_byte_offset))
        .ok_or_else(|| {
            Diagnostic::error("AArch64 MVP encoder cannot address overflowing fixed indexed copy")
        })?;
    append_add_constant_to_x_register(&mut bytes, 16, source_offset)?;
    bytes.extend(encode_load_x_from_x(20, 20, pointer_byte_offset)?);
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

/// Exact scratch footprint when the fixed-indexed encoder above is used for
/// a pointee pair (index zero, element size one). x16/x20 hold the pointees,
/// x17 stages non-empty chunks, and large field/chunk offsets use x19/x26.
pub fn runtime_storage_copy_pointee_pair_clobbers(
    source_field_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(20)];
    if byte_count > 0 {
        registers.push(MachineRegister::Aarch64X(17));
    }
    if source_field_byte_offset > 4095 || target_field_byte_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        if !data_offset_encodable(offset, chunk_size) {
            registers.extend([MachineRegister::Aarch64X(19), MachineRegister::Aarch64X(26)]);
        }
        Ok(())
    })
    .expect("runtime copy chunk partition is total");
    RegisterSet::new(registers)
}

pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region(
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
        byte_count,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_width(
            element_byte_size,
            source_field_byte_offset,
            target_field_byte_offset,
            byte_count,
        ) + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8;
    let mut bytes = Vec::with_capacity(expected_width);
    // x16 = element source-field address (`*(frame[descriptor]) + index*elem +
    // source_field`); leaves x20 = frame base.
    append_runtime_frame_index_target_address_with_index_region(
        &mut bytes,
        16,
        index_region,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        17,
        26,
    )?;
    // x20 = target pointer value (`*(frame[pointer])`), then + target field.
    bytes.extend(encode_load_x_from_x(20, 20, pointer_byte_offset)?);
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage(
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
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
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

/// Exact scratch footprint shared by machine-inline-array element reads. The
/// address recipe always writes x16/x17/x19/x20/x26; large offsets and copied
/// chunks remain within that closed set.
pub fn runtime_storage_copy_from_runtime_machine_indexed_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

/// Exact scratch footprint of the machine-inline-array write-side mirror.
pub fn runtime_storage_copy_to_runtime_machine_indexed_clobbers() -> RegisterSet {
    runtime_storage_copy_from_runtime_machine_indexed_clobbers()
}

/// Copy a machine-rooted inline array element through a frame-held pointer.
/// x16 retains the machine root while x15 supplies the pointer and any
/// frame-held index.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_machine_indexed_to_runtime_pointee(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_machine_indexed_to_runtime_pointee_width(
            pointer_byte_offset,
            target_field_byte_offset,
            byte_count,
        );
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(15));
    bytes.extend(encode_add_page_offset_placeholder(15));
    append_single_index_address_math(
        &mut bytes,
        if index_region == frame { 15 } else { 16 },
        index_offset,
        index_byte_size,
        element_byte_size,
        base_byte_offset + source_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 15, pointer_byte_offset, 8, 19)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_machine_indexed_to_runtime_pointee_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_runtime_pointee_to_machine_indexed(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_runtime_pointee_to_machine_indexed_width(
            pointer_byte_offset,
            source_field_byte_offset,
            byte_count,
        );
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(15));
    bytes.extend(encode_add_page_offset_placeholder(15));
    append_single_index_address_math(
        &mut bytes,
        if index_region == frame { 15 } else { 16 },
        index_offset,
        index_byte_size,
        element_byte_size,
        base_byte_offset + target_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 15, pointer_byte_offset, 8, 19)?;
    append_add_constant_to_x_register(&mut bytes, 20, source_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_runtime_pointee_to_machine_indexed_clobbers() -> RegisterSet {
    runtime_storage_copy_machine_indexed_to_runtime_pointee_clobbers()
}

/// Write-side mirror of
/// [`encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`].
/// x86_64-only for now; aarch64 emits nothing real.
/// Write `machine[index] = <machine-storage source>` — the store-side mirror of
/// `encode_runtime_storage_copy_from_runtime_machine_indexed_to_runtime_storage`.
/// Computes the ELEMENT address (base + index*scale + field) into x16 exactly as
/// the read does, computes the SOURCE address (source region + `source_offset`)
/// into x20, then LOADs from the source (x20) and STOREs into the element (x16) --
/// the load/store bases are swapped relative to the read. The source page pair's
/// SYMBOL is chosen by the relocation record (machine for a field source, the
/// runtime frame for a slot-backed local source); the emitted bytes are
/// identical either way.
pub fn encode_runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage(
    source_offset: usize,
    base_byte_offset: usize,
    index_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_to_runtime_machine_indexed_from_runtime_storage_width(
            source_offset,
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_count,
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
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

pub fn encode_runtime_storage_copy_to_runtime_pointee(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_copy_to_runtime_pointee_width(
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
        byte_count,
    ));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_add_constant_to_x_register(&mut bytes, 20, source_offset)?;
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

/// Exact scratch footprint of the direct-source to runtime-pointee encoder
/// above. Base/result registers are unconditional; x19 participates only when
/// a base adjustment exceeds one ADD immediate, while x19/x26 participate when
/// a chunk offset needs an address scratch.
pub fn runtime_storage_copy_to_runtime_pointee_clobbers(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(20)];
    if byte_count > 0 {
        registers.push(MachineRegister::Aarch64X(17));
    }
    if source_offset > 4095 || pointer_byte_offset > 4095 || field_byte_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        if !data_offset_encodable(offset, chunk_size) {
            registers.extend([MachineRegister::Aarch64X(19), MachineRegister::Aarch64X(26)]);
        }
        Ok(())
    })
    .expect("runtime copy chunk partition is total");
    RegisterSet::new(registers)
}

pub fn encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_storage_copy_from_runtime_pointee_to_runtime_frame_width(
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;

    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;

    Ok(bytes)
}

/// Exact scratch footprint of the dereferenced-source copy encoder above.
/// x16 walks the source pointer, x20 holds the target base, and x17 stages
/// non-empty chunks. x19/x26 are used only when address immediates overflow.
pub fn runtime_storage_copy_from_runtime_pointee_clobbers(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(20)];
    if byte_count > 0 {
        registers.push(MachineRegister::Aarch64X(17));
    }
    if pointer_byte_offset > 4095 || field_byte_offset > 4095 || target_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        if !data_offset_encodable(offset, chunk_size) {
            registers.extend([MachineRegister::Aarch64X(19), MachineRegister::Aarch64X(26)]);
        }
        Ok(())
    })
    .expect("runtime copy chunk partition is total");
    RegisterSet::new(registers)
}

fn for_each_runtime_copy_chunk(
    source_base_offset: usize,
    target_base_offset: usize,
    byte_count: usize,
    mut visit: impl FnMut(usize, usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut remaining = byte_count;
    let mut offset = 0usize;

    while remaining > 0 {
        let source_offset = source_base_offset + offset;
        let target_offset = target_base_offset + offset;
        let chunk_size =
            if remaining >= 8 && source_offset.is_multiple_of(8) && target_offset.is_multiple_of(8)
            {
                8
            } else if remaining >= 4
                && source_offset.is_multiple_of(4)
                && target_offset.is_multiple_of(4)
            {
                4
            } else {
                1
            };

        visit(offset, chunk_size)?;
        offset += chunk_size;
        remaining -= chunk_size;
    }

    if offset != byte_count {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot copy `{byte_count}` byte(s) of runtime storage yet"
        )));
    }

    Ok(())
}

/// Machine-storage flavor: a MACHINE-resident index (a subslice start held in a
/// machine field) materializes its own page pair into a dedicated base scratch
/// at the CONSTANT offset 32 (after the frame pair + the fixed-width descriptor
/// load), which the relocation record patches to the machine symbol.

pub fn encode_runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage(
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
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot read {byte_count}-byte double-indexed values yet"
        )));
    }
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
            outer_index_region,
            inner_index_region,
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
    match byte_count {
        8 => bytes.extend(encode_load_x_from_x(17, 16, 0)?),
        _ => bytes.extend(encode_load_w_from_x(17, 16, 0, byte_count)?),
    }
    // Target base (relocated at `..target_base_offset`, a constant per frame-ness).
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    match byte_count {
        8 => bytes.extend(encode_store_x_to_x(17, 16, target_offset)?),
        _ => bytes.extend(encode_store_w_to_x(17, 16, target_offset, byte_count)?),
    }
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_storage_copy_from_runtime_machine_double_indexed_to_runtime_storage_width(
            outer_index_region,
            inner_index_region,
        )
    );
    Ok(bytes)
}

/// Exact scratch footprint of a machine-rooted double-indexed read. x15 is
/// present exactly when either index slot is frame-resident.
pub fn runtime_storage_copy_from_runtime_machine_double_indexed_clobbers(
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(26),
    ];
    if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Write `grid[i][j] = <storage slot>` -- the source value loads into x24
/// FIRST (right after the base pairs, while x16 is still the unbiased machine
/// base; the shared frame pair also serves a frame-resident SOURCE), then the
/// address math walks x16 to the element and x24 stores there.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_offset: usize,
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if !matches!(byte_count, 1 | 4 | 8) {
        return Err(Diagnostic::error(format!(
            "AArch64 MVP encoder cannot write {byte_count}-byte double-indexed values yet"
        )));
    }
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
            source_region,
            outer_index_region,
            inner_index_region,
        ),
    );
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if source_region == frame || outer_index_region == frame || inner_index_region == frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    let source_base = if source_region == frame { 15 } else { 16 };
    match byte_count {
        8 => bytes.extend(encode_load_x_from_x(24, source_base, source_offset)?),
        _ => bytes.extend(encode_load_w_from_x(
            24,
            source_base,
            source_offset,
            byte_count,
        )?),
    }
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == frame { 15 } else { 16 },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == frame { 15 } else { 16 },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    match byte_count {
        8 => bytes.extend(encode_store_x_to_x(24, 16, 0)?),
        _ => bytes.extend(encode_store_w_to_x(24, 16, 0, byte_count)?),
    }
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_storage_copy_to_runtime_machine_double_indexed_from_runtime_storage_width(
            source_region,
            outer_index_region,
            inner_index_region,
        )
    );
    Ok(bytes)
}

/// Exact scratch footprint of a double-indexed machine-array write. x15 is
/// included exactly when the source or either index is frame-resident.
pub fn runtime_storage_copy_to_runtime_machine_double_indexed_clobbers(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ];
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    if source_region == frame || outer_index_region == frame || inner_index_region == frame {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Read `g[i][j]` from a FRAME-resident 2D array (a `let`/param local): one
/// frame pair serves the array and frame-held indices, one optional machine
/// pair serves either or both machine-held indices, then the relocated target
/// pair and an exact copy of the complete value representation.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_storage_width(
            outer_index_region,
            inner_index_region,
            target_offset,
            byte_count,
        );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16)); // frame base [reloc @ 0]
    bytes.extend(encode_add_page_offset_placeholder(16));
    if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        bytes.extend(encode_adrp_placeholder(15)); // machine indices [reloc @ 8]
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    bytes.extend(encode_adrp_placeholder(20)); // target base [reloc @ 44/52]
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_add_constant_to_x_register(&mut bytes, 20, target_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

/// Exact scratch footprint of a frame-inline double-indexed element read.
pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_clobbers(
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

/// Copy an all-frame single-indexed element through a target pointer held in
/// the same frame. One relocated root supplies the collection, index, and
/// pointer slot; the pointee itself is reached through the loaded pointer.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    source_field_byte_offset: usize,
    pointer_byte_offset: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_width(
        index_region,
        pointer_byte_offset,
        target_field_byte_offset,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_single_index_address_math(
        &mut bytes,
        if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        index_offset,
        index_byte_size,
        element_byte_size,
        base_byte_offset + source_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 20, pointer_byte_offset, 8, 15)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_clobbers()
-> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

/// Copy through a frame-held source pointer into an all-frame single-indexed
/// element. The pointer slot, collection, and index share one frame root.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed(
    pointer_byte_offset: usize,
    source_field_byte_offset: usize,
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = crate::aarch64::widths::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_width(
        index_region,
        pointer_byte_offset,
        source_field_byte_offset,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_single_index_address_math(
        &mut bytes,
        if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        index_offset,
        index_byte_size,
        element_byte_size,
        base_byte_offset + target_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 20, pointer_byte_offset, 8, 15)?;
    append_add_constant_to_x_register(&mut bytes, 20, source_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_clobbers()
-> RegisterSet {
    runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_clobbers()
}

/// Copy a frame-inline double-indexed element through a target pointer held in
/// the same frame. One relocated frame root supplies the collection, pointer
/// slot, and frame-held indices; one additional machine root supplies either
/// or both machine-held indices. The pointee itself is reached through the
/// loaded pointer value.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_width(
        outer_index_region,
        inner_index_region,
        pointer_byte_offset,
        target_field_byte_offset,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + source_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 20, pointer_byte_offset, 8, 15)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_clobbers()
-> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

/// Copy a machine-rooted inline 2D array element through a pointer held in
/// the runtime frame. The machine root walks x16; one frame root in x15 serves
/// every frame-held index and the pointer slot.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_machine_double_indexed_to_runtime_pointee(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_machine_double_indexed_to_runtime_pointee_width(
            pointer_byte_offset,
            target_field_byte_offset,
            byte_count,
        );
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(15));
    bytes.extend(encode_add_page_offset_placeholder(15));
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == frame { 15 } else { 16 },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == frame { 15 } else { 16 },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + source_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 15, pointer_byte_offset, 8, 19)?;
    append_add_constant_to_x_register(&mut bytes, 20, target_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 16, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 20, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_machine_double_indexed_to_runtime_pointee_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ])
}

/// Copy through a frame-held pointer into a machine-rooted inline 2D array
/// element. The target machine root opens the instruction; one frame root
/// supplies the source pointer and any frame-held indices.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_runtime_pointee_to_machine_double_indexed(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_runtime_pointee_to_machine_double_indexed_width(
            pointer_byte_offset,
            source_field_byte_offset,
            byte_count,
        );
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(15));
    bytes.extend(encode_add_page_offset_placeholder(15));
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == frame { 15 } else { 16 },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == frame { 15 } else { 16 },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + target_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 15, pointer_byte_offset, 8, 19)?;
    append_add_constant_to_x_register(&mut bytes, 20, source_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_runtime_pointee_to_machine_double_indexed_clobbers() -> RegisterSet {
    runtime_storage_copy_machine_double_indexed_to_runtime_pointee_clobbers()
}

/// Copy through a frame-held source pointer into a frame-inline double-indexed
/// element. One relocated frame root supplies the pointer slot, collection,
/// and frame-held indices; one additional machine root supplies either or both
/// machine-held indices. The pointee itself is reached through the loaded
/// pointer value.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = crate::aarch64::widths::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_width(
        outer_index_region,
        inner_index_region,
        pointer_byte_offset,
        source_field_byte_offset,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + target_field_byte_offset,
    )?;
    append_load_data_from_x_offset(&mut bytes, 20, 20, pointer_byte_offset, 8, 15)?;
    append_add_constant_to_x_register(&mut bytes, 20, source_field_byte_offset)?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 20, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_clobbers()
-> RegisterSet {
    runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_clobbers()
}

/// Write a direct storage slot into frame-inline `g[i][j]`. The target frame
/// pair leads the instruction and remains in x16; x20 preserves a frame source
/// while one x15 pair supplies a machine source and/or machine-held indices.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage(
    source_region: omega_target_operations::RuntimeStorageRegion,
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = crate::aarch64::widths::runtime_storage_copy_to_runtime_frame_base_double_indexed_from_runtime_storage_width(
        source_region,
        outer_index_region,
        inner_index_region,
        source_offset,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16)); // target frame base [reloc @ 0]
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    if source_region == omega_target_operations::RuntimeStorageRegion::Machine
        || outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        bytes.extend(encode_adrp_placeholder(15)); // shared machine base [reloc @ 12]
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    let source_base =
        if source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
            20
        } else {
            15
        };
    append_double_index_address_math(
        &mut bytes,
        if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        if inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine {
            15
        } else {
            16
        },
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(source_offset, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(
            &mut bytes,
            24,
            source_base,
            source_offset + offset,
            chunk_size,
            19,
        )?;
        append_store_data_to_x_offset(&mut bytes, 24, 16, offset, chunk_size, 26)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_to_runtime_frame_base_double_indexed_clobbers(
    source_region: omega_target_operations::RuntimeStorageRegion,
    outer_index_region: omega_target_operations::RuntimeStorageRegion,
    inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ];
    if source_region == omega_target_operations::RuntimeStorageRegion::Machine
        || outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
        || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
    {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Copy a FRAME-resident inline array element at a runtime index into another
/// frame slot (`let v = arr[i]` where `arr` and `i` are locals/params): ONE
/// frame pair serves the element address, the index, and the target slot --
/// the unbiased base is stashed in x24 before the element math biases x16, and
/// the chunk stores land at `[x24 + target_offset + chunk]`. Single relocation
/// (the record's arch-aware target-frame offset is None for aarch64).
pub fn encode_runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame_width(
            target_offset,
            byte_count,
        ),
    );
    bytes.extend(encode_adrp_placeholder(16)); // frame base [reloc @ 0]
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(24, 16)); // unbiased base for the target
    // The index lives in the SAME region as the base, so the fixed-shape
    // recipe's same-region flavor (no extra page pair) applies.
    append_fixed_shape_index_element_address(
        &mut bytes,
        16,
        omega_target_operations::RuntimeStorageRegion::Machine,
        index_offset,
        index_byte_size,
        element_byte_size,
        base_byte_offset + field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, target_offset, byte_count, |offset, chunk_size| {
        let source_offset = offset;
        let target_chunk_offset = target_offset + offset;
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 16, source_offset)?);
                bytes.extend(encode_store_x_to_x(17, 24, target_chunk_offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 16, source_offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(
                    17,
                    24,
                    target_chunk_offset,
                    chunk_size,
                )?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_frame_width(
            target_offset,
            byte_count,
        )
    );
    Ok(bytes)
}

/// Copy `machine[j] -> machine[i]` where BOTH indices are runtime values
/// (`arr[i] = arr[j]`): compute the source element address (fixed shape,
/// stashed in x24), compute the target element address (a second relocated
/// machine base), then chunk-copy through x17. Historically this op was
/// silently DROPPED on aarch64 (the zero-width emission hole); the layout
/// guard now makes any regression loud.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_machine_indexed_to_machine_indexed(
    source_base_byte_offset: usize,
    source_index_offset: usize,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    source_index_byte_size: usize,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_base_byte_offset: usize,
    target_index_offset: usize,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_byte_size: usize,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_storage_copy_machine_indexed_to_machine_indexed_width(
            source_index_region,
            target_index_region,
            byte_count,
        ),
    );
    // Source element address -> x16 -> stash x24.
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_fixed_shape_index_element_address(
        &mut bytes,
        16,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    // Target element address -> x16 (the second relocated machine base sits at
    // `..second_base_offset`, a region-dependent constant).
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_fixed_shape_index_element_address(
        &mut bytes,
        16,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    // Chunk-copy source (x24) -> target (x16) through x17.
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 24, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 24, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_storage_copy_machine_indexed_to_machine_indexed_width(
            source_index_region,
            target_index_region,
            byte_count,
        )
    );
    Ok(bytes)
}

/// Exact scratch footprint of `machine[i] = machine[j]`. x16 walks each
/// machine element, x20 selects each index base, x17/x26 load and scale the
/// indices and later stage copy chunks, and x24 preserves the source address
/// while the target address is materialized.
pub fn runtime_storage_copy_machine_indexed_to_machine_indexed_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ])
}

/// Copy `frame[j] -> frame[i]` between frame-inline arrays. x20 preserves the
/// relocated frame root, one optional x15 root supplies every machine-held
/// index, and x24 preserves the completed source address between walks.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_frame_base_indexed_to_frame_base_indexed(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_width(
            source_index_region,
            target_index_region,
            byte_count,
        );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    if source_index_region == machine || target_index_region == machine {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_single_index_address_math(
        &mut bytes,
        if source_index_region == machine {
            15
        } else {
            20
        },
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    bytes.extend(encode_move_x_register(16, 20));
    bytes.extend(encode_move_x_register(20, 16));
    append_single_index_address_math(
        &mut bytes,
        if target_index_region == machine {
            15
        } else {
            20
        },
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 24, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 24, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_clobbers(
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = runtime_storage_copy_machine_indexed_to_machine_indexed_clobbers()
        .as_slice()
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    if source_index_region == machine || target_index_region == machine {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Copy one single-indexed inline-array element between machine and frame
/// storage. x16 starts from the source collection root, x15 retains the target
/// collection root, x20 preserves the source root for either index, and x24
/// preserves the completed source address.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_cross_region_indexed_pair(
    source_region: omega_target_operations::RuntimeStorageRegion,
    source_base_byte_offset: usize,
    source_index_region: omega_target_operations::RuntimeStorageRegion,
    source_index_offset: usize,
    source_index_byte_size: usize,
    source_element_byte_size: usize,
    source_field_byte_offset: usize,
    target_region: omega_target_operations::RuntimeStorageRegion,
    target_base_byte_offset: usize,
    target_index_region: omega_target_operations::RuntimeStorageRegion,
    target_index_offset: usize,
    target_index_byte_size: usize,
    target_element_byte_size: usize,
    target_field_byte_offset: usize,
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if source_region == target_region {
        return Err(Diagnostic::error(
            "cross-region indexed pair copy requires distinct source and target roots",
        ));
    }
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_cross_region_indexed_pair_width(byte_count);
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(15));
    bytes.extend(encode_add_page_offset_placeholder(15));
    bytes.extend(encode_move_x_register(20, 16));
    append_single_index_address_math(
        &mut bytes,
        if source_index_region == source_region {
            20
        } else {
            15
        },
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    bytes.extend(encode_move_x_register(16, 15));
    append_single_index_address_math(
        &mut bytes,
        if target_index_region == target_region {
            15
        } else {
            20
        },
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 24, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 24, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_cross_region_indexed_pair_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ])
}

/// Double-indexed aggregate copy across one machine-inline and one frame-inline
/// array. The source and target roots are materialized once and reused by all
/// four independently placed indices.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_cross_region_double_indexed_pair(
    source_region: omega_target_operations::RuntimeStorageRegion,
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
    target_region: omega_target_operations::RuntimeStorageRegion,
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if source_region == target_region {
        return Err(Diagnostic::error(
            "cross-region double-indexed pair copy requires distinct roots",
        ));
    }
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_cross_region_double_indexed_pair_width(
            byte_count,
        );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(15));
    bytes.extend(encode_add_page_offset_placeholder(15));
    bytes.extend(encode_move_x_register(20, 16));
    let source_index_base = |region| if region == source_region { 20 } else { 15 };
    append_double_index_address_math(
        &mut bytes,
        source_index_base(source_outer_index_region),
        source_outer_index_offset,
        source_outer_index_byte_size,
        source_outer_stride,
        source_index_base(source_inner_index_region),
        source_inner_index_offset,
        source_inner_index_byte_size,
        source_inner_stride,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    bytes.extend(encode_move_x_register(16, 15));
    let target_index_base = |region| if region == target_region { 15 } else { 20 };
    append_double_index_address_math(
        &mut bytes,
        target_index_base(target_outer_index_region),
        target_outer_index_offset,
        target_outer_index_byte_size,
        target_outer_stride,
        target_index_base(target_inner_index_region),
        target_inner_index_offset,
        target_inner_index_byte_size,
        target_inner_stride,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        match chunk_size {
            8 => {
                bytes.extend(encode_load_x_from_x(17, 24, offset)?);
                bytes.extend(encode_store_x_to_x(17, 16, offset)?);
            }
            _ => {
                bytes.extend(encode_load_w_from_x(17, 24, offset, chunk_size)?);
                bytes.extend(encode_store_w_to_x(17, 16, offset, chunk_size)?);
            }
        }
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_cross_region_double_indexed_pair_clobbers() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ])
}

/// Copy `frame[source_outer][source_inner]` to
/// `frame[target_outer][target_inner]`. x20 preserves the relocated frame root,
/// one optional x15 root supplies every machine-held index, and x24 preserves
/// the completed source address between walks.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width = crate::aarch64::widths::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_width(
        source_outer_index_region,
        source_inner_index_region,
        target_outer_index_region,
        target_inner_index_region,
        byte_count,
    );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    if source_outer_index_region == machine
        || source_inner_index_region == machine
        || target_outer_index_region == machine
        || target_inner_index_region == machine
    {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if source_outer_index_region == machine {
            15
        } else {
            16
        },
        source_outer_index_offset,
        source_outer_index_byte_size,
        source_outer_stride,
        if source_inner_index_region == machine {
            15
        } else {
            16
        },
        source_inner_index_offset,
        source_inner_index_byte_size,
        source_inner_stride,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    bytes.extend(encode_move_x_register(16, 20));
    append_double_index_address_math(
        &mut bytes,
        if target_outer_index_region == machine {
            15
        } else {
            16
        },
        target_outer_index_offset,
        target_outer_index_byte_size,
        target_outer_stride,
        if target_inner_index_region == machine {
            15
        } else {
            16
        },
        target_inner_index_offset,
        target_inner_index_byte_size,
        target_inner_stride,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 24, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_clobbers(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
    target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ];
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    if source_outer_index_region == machine
        || source_inner_index_region == machine
        || target_outer_index_region == machine
        || target_inner_index_region == machine
    {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}

/// Copy between two machine-rooted inline 2D array elements. Each address
/// walk begins from its own relocated machine root; a side whose outer or
/// inner runtime index is frame-held materializes one frame root in x15.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_storage_copy_machine_double_indexed_to_machine_double_indexed(
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
    byte_count: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_width(
            source_outer_index_region,
            source_inner_index_region,
            target_outer_index_region,
            target_inner_index_region,
            byte_count,
        );
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if source_outer_index_region == frame || source_inner_index_region == frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if source_outer_index_region == frame {
            15
        } else {
            16
        },
        source_outer_index_offset,
        source_outer_index_byte_size,
        source_outer_stride,
        if source_inner_index_region == frame {
            15
        } else {
            16
        },
        source_inner_index_offset,
        source_inner_index_byte_size,
        source_inner_stride,
        source_base_byte_offset + source_field_byte_offset,
    )?;
    bytes.extend(encode_move_x_register(24, 16));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    if target_outer_index_region == frame || target_inner_index_region == frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_double_index_address_math(
        &mut bytes,
        if target_outer_index_region == frame {
            15
        } else {
            16
        },
        target_outer_index_offset,
        target_outer_index_byte_size,
        target_outer_stride,
        if target_inner_index_region == frame {
            15
        } else {
            16
        },
        target_inner_index_offset,
        target_inner_index_byte_size,
        target_inner_stride,
        target_base_byte_offset + target_field_byte_offset,
    )?;
    for_each_runtime_copy_chunk(0, 0, byte_count, |offset, chunk_size| {
        append_load_data_from_x_offset(&mut bytes, 17, 24, offset, chunk_size, 26)?;
        append_store_data_to_x_offset(&mut bytes, 17, 16, offset, chunk_size, 19)?;
        Ok(())
    })?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_clobbers(
    source_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    source_inner_index_region: omega_target_operations::RuntimeStorageRegion,
    target_outer_index_region: omega_target_operations::RuntimeStorageRegion,
    target_inner_index_region: omega_target_operations::RuntimeStorageRegion,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(24),
        MachineRegister::Aarch64X(26),
    ];
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    if source_outer_index_region == frame
        || source_inner_index_region == frame
        || target_outer_index_region == frame
        || target_inner_index_region == frame
    {
        registers.push(MachineRegister::Aarch64X(15));
    }
    RegisterSet::new(registers)
}
