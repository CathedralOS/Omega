use omega_calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

use super::{
    append_add_constant_to_x_register, append_double_index_address_math, append_double_index_bases,
    append_runtime_frame_base_index_target_address_with_index_region,
    append_runtime_frame_index_target_address_with_index_region,
    append_runtime_machine_index_target_address, append_runtime_storage_load,
    append_store_data_to_x_offset,
};
use crate::aarch64::primitives::{
    append_unsigned_immediate, encode_add_page_offset_placeholder, encode_adrp_placeholder,
    encode_store_x_to_x, encode_store_x17_to_x16,
};
use crate::aarch64::widths::{
    runtime_frame_base_indexed_string_write_with_index_region_width,
    runtime_frame_indexed_string_write_width_with_index_region, runtime_frame_string_write_width,
    runtime_machine_indexed_string_write_width_with_index_region,
    runtime_machine_string_write_width, runtime_pointee_string_write_width,
};

pub fn encode_runtime_machine_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_string_write_width(byte_length) + 40);
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_store_data_to_x_offset(&mut bytes, 17, 16, byte_offset, 8, 15)?;
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    append_store_data_to_x_offset(&mut bytes, 17, 16, byte_offset + 8, 8, 15)?;
    Ok(bytes)
}

pub fn encode_runtime_frame_string_write(
    byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_string_write_width(byte_length));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x17_to_x16(byte_offset + 8)?);
    Ok(bytes)
}

/// Store one relocated immutable-data address into a direct runtime-frame
/// pointer word without manufacturing an adjacent string length.
pub fn encode_runtime_frame_data_address_write(byte_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_storage_function_address_write(byte_offset)
}

/// Store one relocated compiler-private function address into a direct
/// runtime-storage pointer word. Region selection changes only the destination
/// relocation target, not this architecture-native instruction program.
pub fn encode_runtime_storage_function_address_write(
    byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(20);
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_store_x17_to_x16(byte_offset)?);
    Ok(bytes)
}

pub fn encode_runtime_pointee_string_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_string_write_width(
        pointer_byte_offset,
        field_byte_offset,
        byte_length,
    ));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee",
    )?;
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_frame_indexed_string_write(
    descriptor_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_indexed_string_write_with_index_region(
        descriptor_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

pub fn encode_runtime_frame_indexed_string_write_with_index_region(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_frame_indexed_string_write_width_with_index_region(
        index_region,
        element_byte_size,
        field_byte_offset,
        byte_length,
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
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_indexed_string_write_width_with_index_region(
            index_region,
            element_byte_size,
            field_byte_offset,
            byte_length,
        )
    );
    Ok(bytes)
}

pub fn encode_runtime_frame_base_indexed_string_write(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_base_indexed_string_write_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_string_write_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_frame_base_indexed_string_write_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_length,
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
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_frame_base_indexed_string_write_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_length,
        )
    );
    Ok(bytes)
}

pub fn encode_runtime_machine_indexed_string_write(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_machine_indexed_string_write_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        byte_length,
    )
}

pub fn encode_runtime_machine_indexed_string_write_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_machine_indexed_string_write_width_with_index_region(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_length,
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
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_indexed_string_write_width_with_index_region(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_length,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_string_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_frame_base_double_indexed_string_write_width(byte_length);
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
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_string_write(
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
    byte_length: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_double_indexed_string_write_width(
            outer_index_region,
            inner_index_region,
            byte_length,
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
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    append_unsigned_immediate(&mut bytes, 17, byte_length as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 8)?);
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_double_indexed_string_write_width(
            outer_index_region,
            inner_index_region,
            byte_length,
        )
    );
    Ok(bytes)
}

/// Closed may-write ceiling of the retained string-descriptor write shapes.
/// It unions the direct/pointee offset scratches with the frame- and
/// machine-indexed address recipe while x17 carries data and length.
pub fn place_string_write_register_write_ceiling() -> RegisterSet {
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

pub const fn place_string_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

#[cfg(test)]
mod data_address_tests {
    use super::*;

    #[test]
    fn runtime_frame_data_address_write_owns_one_word_and_two_bases() {
        let bytes = encode_runtime_frame_data_address_write(40).expect("direct data-address write");
        assert_eq!(bytes.len(), 20);
        assert_eq!(
            &bytes[..8],
            &[0x11, 0x00, 0x00, 0x90, 0x31, 0x02, 0x00, 0x91]
        );
        assert_eq!(
            &bytes[8..16],
            &[0x10, 0x00, 0x00, 0x90, 0x10, 0x02, 0x00, 0x91]
        );
    }

    #[test]
    fn callback_function_address_store_has_two_unpatched_address_pairs() {
        let bytes = encode_runtime_storage_function_address_write(40).unwrap();
        assert_eq!(bytes.len(), 20);
        assert_eq!(
            &bytes[..16],
            &[
                0x11, 0x00, 0x00, 0x90, 0x31, 0x02, 0x00, 0x91, 0x10, 0x00, 0x00, 0x90, 0x10, 0x02,
                0x00, 0x91,
            ]
        );
    }
}
