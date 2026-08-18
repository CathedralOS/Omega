use omega_calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

use super::{
    append_add_constant_to_x_register, append_double_index_address_math, append_double_index_bases,
    append_runtime_frame_base_index_target_address_with_index_region,
    append_runtime_frame_fixed_index_target_address,
    append_runtime_frame_index_target_address_with_index_region,
    append_runtime_machine_index_target_address, append_runtime_storage_load,
    append_store_data_to_x_offset, append_store_x_to_x_offset,
};
use crate::aarch64::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_move_x_register,
};
use crate::aarch64::widths::{
    runtime_frame_fixed_indexed_address_to_runtime_frame_write_width,
    runtime_frame_indexed_address_to_runtime_frame_write_width,
    runtime_pointee_address_to_runtime_frame_write_width,
    runtime_storage_address_to_runtime_frame_write_width,
};

pub fn encode_runtime_storage_address_to_runtime_frame_write(
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_storage_address_to_runtime_frame_write_width(
        source_offset,
        target_offset,
    ));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_add_constant_to_x_register(&mut bytes, 17, source_offset)?;
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_store_x_to_x_offset(&mut bytes, 17, 16, target_offset)?;
    Ok(bytes)
}

pub fn runtime_storage_address_to_runtime_frame_write_clobbers(
    source_offset: usize,
    target_offset: usize,
) -> RegisterSet {
    let mut registers = vec![MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)];
    if source_offset > 4095 || target_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    RegisterSet::new(registers)
}

pub fn encode_runtime_pointee_address_to_runtime_frame_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_address_to_runtime_frame_write_width(
        pointer_byte_offset,
        field_byte_offset,
        target_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_move_x_register(20, 16));
    append_runtime_storage_load(
        &mut bytes,
        17,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    append_add_constant_to_x_register(&mut bytes, 17, field_byte_offset)?;
    append_store_x_to_x_offset(&mut bytes, 17, 20, target_offset)?;
    Ok(bytes)
}

pub fn runtime_pointee_address_to_runtime_frame_write_clobbers(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(20),
    ];
    if pointer_byte_offset > 4095 || field_byte_offset > 4095 || target_offset > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    RegisterSet::new(registers)
}

pub fn encode_runtime_frame_indexed_address_to_runtime_frame_write(
    index_region: omega_target_operations::RuntimeStorageRegion,
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_address_to_runtime_frame_write_width(
        index_region,
        element_byte_size,
        field_byte_offset,
        target_offset,
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
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    Ok(bytes)
}

pub fn runtime_frame_indexed_address_to_runtime_frame_write_clobbers(
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

pub fn encode_runtime_frame_fixed_indexed_address_to_runtime_frame_write(
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_frame_fixed_indexed_address_to_runtime_frame_write_width(
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
        ),
    );
    append_runtime_frame_fixed_index_target_address(
        &mut bytes,
        16,
        descriptor_offset,
        element_index,
        element_byte_size,
        field_byte_offset,
    )?;
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_frame_base_indexed_address_to_runtime_frame_write_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
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
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    Ok(bytes)
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers() -> RegisterSet {
    runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers_with_index_region(
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
    )
}

pub fn runtime_frame_base_indexed_address_to_runtime_frame_write_clobbers_with_index_region(
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

/// Materialize the address of `grid[i][j]` when the inline 2D array and both
/// runtime index slots share the frame, then store that address in a frame-held
/// reference slot. X20 retains the unbiased frame base while x16 walks to the
/// element, so one relocated base pair serves the entire operation.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_frame_base_double_indexed_address_to_runtime_frame_write_width(
            target_offset,
        );
    let mut bytes = Vec::with_capacity(expected_width);
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_move_x_register(16, 20));
    append_double_index_address_math(
        &mut bytes,
        20,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        20,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        base_byte_offset + field_byte_offset,
    )?;
    append_store_x_to_x_offset(&mut bytes, 16, 20, target_offset)?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_frame_base_double_indexed_address_to_runtime_frame_write_clobbers(
    target_offset: usize,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ];
    if !target_offset.is_multiple_of(8) || target_offset / 8 > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    RegisterSet::new(registers)
}

/// The machine-indexed ADDRESS write: `frame[target] = &machine[base + idx*size
/// + field]` -- the SS5b wide-referee recast (`&self.buf[k] as &Wide`) binds the
/// frame slot to the ELEMENT ADDRESS (reads deref it; a wider-than-pointer
/// referee cannot content-spill). The address computation is the SAME prefix as
/// the machine-indexed copies (`append_runtime_machine_index_target_address`
/// into x16, machine page pair relocated at instruction start + the frame
/// index's own page pair for a RuntimeFrame index), so the relocation walker
/// reuses the copy family's offset fns; then the target frame page pair (x17)
/// and an 8-byte store of x16 (scratch x9 materializes a large target offset).
pub fn encode_runtime_machine_indexed_address_to_runtime_frame_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_indexed_address_to_runtime_frame_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
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
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_store_data_to_x_offset(&mut bytes, 16, 17, target_offset, 8, 9)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_indexed_address_to_runtime_frame_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            target_offset,
        )
    );
    Ok(bytes)
}

pub fn runtime_machine_indexed_address_to_runtime_frame_write_clobbers(
    target_offset: usize,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(19),
        MachineRegister::Aarch64X(20),
        MachineRegister::Aarch64X(26),
    ];
    if !target_offset.is_multiple_of(8) || target_offset / 8 > 4095 {
        registers.push(MachineRegister::Aarch64X(9));
    }
    RegisterSet::new(registers)
}

/// Materialize a machine-owned `grid[i][j]` address and store it in a frame-
/// held reference slot. A frame page pair loaded for either index is reused for
/// the destination; when both indices are machine-held, that pair is emitted
/// after the fixed element-address program.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_address_to_runtime_frame_write(
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
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_machine_double_indexed_address_to_runtime_frame_write_width(
            target_offset,
        );
    let mut bytes = Vec::with_capacity(expected_width);
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
    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
    if outer_index_region != frame && inner_index_region != frame {
        bytes.extend(encode_adrp_placeholder(15));
        bytes.extend(encode_add_page_offset_placeholder(15));
    }
    append_store_x_to_x_offset(&mut bytes, 16, 15, target_offset)?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

pub fn runtime_machine_double_indexed_address_to_runtime_frame_write_clobbers(
    target_offset: usize,
) -> RegisterSet {
    let mut registers = vec![
        MachineRegister::Aarch64X(14),
        MachineRegister::Aarch64X(15),
        MachineRegister::Aarch64X(16),
        MachineRegister::Aarch64X(17),
        MachineRegister::Aarch64X(26),
    ];
    if !target_offset.is_multiple_of(8) || target_offset / 8 > 4095 {
        registers.push(MachineRegister::Aarch64X(19));
    }
    RegisterSet::new(registers)
}

pub fn runtime_place_address_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}
