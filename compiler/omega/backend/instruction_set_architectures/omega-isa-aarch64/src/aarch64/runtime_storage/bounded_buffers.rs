use omega_calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

use super::{
    append_add_constant_to_x_register, append_double_index_address_math, append_double_index_bases,
    append_runtime_frame_base_index_target_address_with_index_region,
    append_runtime_frame_index_target_address_with_index_region,
    append_runtime_machine_index_target_address, append_runtime_storage_load,
};
use crate::aarch64::primitives::{
    append_add_x_constant, append_unsigned_immediate, encode_add_page_offset_placeholder,
    encode_add_x_immediate, encode_add_x_register, encode_adrp_placeholder, encode_cbz_x,
    encode_load_byte_w_post_increment, encode_load_x_from_x, encode_store_byte_w_post_increment,
    encode_store_byte_w_to_x, encode_store_x_to_x, encode_subs_x_immediate,
    encode_unconditional_branch,
};
use crate::aarch64::widths::{
    runtime_machine_bounded_buffer_literal_append_width,
    runtime_machine_bounded_buffer_source_append_width, runtime_machine_bounded_buffer_write_width,
    runtime_pointee_bounded_buffer_write_width,
};

/// Write a string literal into an owned `[u8; N]` byte carrier held directly in
/// machine storage (`self.buffer = "150"`). The carrier is `{ len: u64, bytes:
/// [u8; N] }`: store the length word at `[base + off]`, then each literal byte
/// inline at `[base + off + 8 + i]`. Content is immediate, so the base --
/// materialized by the leading `adrp`+`add` placeholder pair, patched to the
/// machine storage base by the relocation pass (the single
/// `insert_data_address_at_instruction_start` reloc, arch-shared with the string
/// writes) -- is the only relocation, mirroring the x86_64 carrier write.
///
/// Every emitted element is a fixed 4-byte AArch64 instruction (immediates live
/// in the instruction word, not as inline data bytes), so the sequence is
/// inherently instruction-aligned. Previously this op errored at encode while its
/// layout width borrowed the x86_64 (odd) width, so a forward branch skipping the
/// block that contained it computed a non-4-aligned distance and failed with a
/// misleading "b.ne target is not instruction aligned".
pub fn encode_runtime_machine_bounded_buffer_write(
    byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_write_width(
        byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = machine storage base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    // Materialize the carrier address once. STRB's unsigned immediate tops out
    // at 4095 bytes, while large machines routinely place text carriers later
    // in storage; rebasing also keeps every per-byte store small.
    append_add_x_constant(&mut bytes, 16, 16, byte_offset, 15)?;
    append_unsigned_immediate(&mut bytes, 17, literal.len() as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 0)?); // [carrier] = len word
    for (index, byte) in literal.iter().enumerate() {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_to_x(17, 16, 8 + index)?);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_write_width(byte_offset, literal)
    );
    Ok(bytes)
}

/// Write a string literal into an owned `[u8; N]` carrier reached THROUGH a
/// stored pointer (`rooms[0].label = "Gate"`): load the pointer from
/// `frame[pointer_byte_offset]` into x16, then store `{len, bytes}` inline at
/// `*ptr + field`. Content is immediate, so the frame base (the leading
/// `adrp`+`add`, relocated at instruction start) is the only relocation --
/// mirroring the x86_64 pointee carrier write.
pub fn encode_runtime_pointee_bounded_buffer_write(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_pointee_bounded_buffer_write_width(
        pointer_byte_offset,
        field_byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = frame base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    append_runtime_storage_load(
        &mut bytes,
        16,
        16,
        pointer_byte_offset,
        8,
        "runtime pointee carrier",
    )?; // x16 = stored pointer
    if field_byte_offset > 0 {
        append_add_constant_to_x_register(&mut bytes, 16, field_byte_offset)?;
    }
    append_unsigned_immediate(&mut bytes, 17, literal.len() as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 0)?); // [*ptr + field] = len word
    for (index, byte) in literal.iter().enumerate() {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_to_x(17, 16, 8 + index)?);
    }
    debug_assert_eq!(
        bytes.len(),
        runtime_pointee_bounded_buffer_write_width(pointer_byte_offset, field_byte_offset, literal)
    );
    Ok(bytes)
}

fn append_bounded_buffer_literal_at_x16(
    bytes: &mut Vec<u8>,
    literal: &[u8],
) -> Result<(), Diagnostic> {
    append_unsigned_immediate(bytes, 17, literal.len() as u64);
    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    for (index, byte) in literal.iter().enumerate() {
        append_unsigned_immediate(bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_to_x(17, 16, 8 + index)?);
    }
    Ok(())
}

fn append_bounded_buffer_literal_to_x16(
    bytes: &mut Vec<u8>,
    literal: &[u8],
) -> Result<(), Diagnostic> {
    bytes.extend(encode_load_x_from_x(15, 16, 0)?);
    append_add_x_constant(bytes, 14, 16, 8, 13)?;
    bytes.extend(encode_add_x_register(14, 14, 15));
    for byte in literal {
        append_unsigned_immediate(bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_post_increment(17, 14, 1)?);
    }
    bytes.extend(encode_add_x_immediate(15, 15, literal.len())?);
    bytes.extend(encode_store_x_to_x(15, 16, 0)?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_bounded_buffer_write(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_frame_indexed_bounded_buffer_write_width(
            index_region,
            element_byte_size,
            field_byte_offset,
            literal,
        ),
    );
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
    append_bounded_buffer_literal_at_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_frame_indexed_bounded_buffer_write_width(
            index_region,
            element_byte_size,
            field_byte_offset,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_bounded_buffer_write(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_base_indexed_bounded_buffer_write_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        literal,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_bounded_buffer_write_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_frame_base_indexed_bounded_buffer_write_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
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
    append_bounded_buffer_literal_at_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_frame_base_indexed_bounded_buffer_write_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_bounded_buffer_write(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_indexed_bounded_buffer_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
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
    append_bounded_buffer_literal_at_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_indexed_bounded_buffer_write_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_bounded_buffer_write(
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
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_double_indexed_bounded_buffer_write_width(
            outer_index_region,
            inner_index_region,
            literal,
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
    append_bounded_buffer_literal_at_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_double_indexed_bounded_buffer_write_width(
            outer_index_region,
            inner_index_region,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_bounded_buffer_write(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_frame_base_double_indexed_bounded_buffer_write_width(
            literal,
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
    append_bounded_buffer_literal_at_x16(&mut bytes, literal)?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_bounded_buffer_literal_append(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let expected_width =
        crate::aarch64::widths::runtime_frame_base_double_indexed_bounded_buffer_literal_append_width(
            literal,
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
    append_bounded_buffer_literal_to_x16(&mut bytes, literal)?;
    debug_assert_eq!(bytes.len(), expected_width);
    Ok(bytes)
}

/// Closed may-write ceiling shared by every classified immediate bounded-buffer
/// encoder. x16 owns the destination and x17 carries length/bytes; the other
/// registers cover the fixed indexed and large-offset address recipes.
#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_bounded_buffer_literal_append(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_frame_indexed_bounded_buffer_literal_append_width(
            index_region,
            element_byte_size,
            field_byte_offset,
            literal,
        ),
    );
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
    append_bounded_buffer_literal_to_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_frame_indexed_bounded_buffer_literal_append_width(
            index_region,
            element_byte_size,
            field_byte_offset,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_bounded_buffer_literal_append(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        literal,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
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
    append_bounded_buffer_literal_to_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_frame_base_indexed_bounded_buffer_literal_append_with_index_region_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_bounded_buffer_literal_append(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_indexed_bounded_buffer_literal_append_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
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
    append_bounded_buffer_literal_to_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_indexed_bounded_buffer_literal_append_width(
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_bounded_buffer_literal_append(
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
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        crate::aarch64::widths::runtime_machine_double_indexed_bounded_buffer_literal_append_width(
            outer_index_region,
            inner_index_region,
            literal,
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
    append_bounded_buffer_literal_to_x16(&mut bytes, literal)?;
    debug_assert_eq!(
        bytes.len(),
        crate::aarch64::widths::runtime_machine_double_indexed_bounded_buffer_literal_append_width(
            outer_index_region,
            inner_index_region,
            literal,
        )
    );
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_double_indexed_bounded_buffer_source_append(
    base_byte_offset: usize,
    outer_index_offset: usize,
    outer_index_byte_size: usize,
    outer_stride: usize,
    inner_index_offset: usize,
    inner_index_byte_size: usize,
    inner_stride: usize,
    field_byte_offset: usize,
    source: &omega_target_operations::Place,
) -> Result<
    (
        Vec<u8>,
        crate::aarch64::place_bounded_buffer::BoundedBufferPlaceSites,
    ),
    Diagnostic,
> {
    let mut bytes = Vec::new();
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
    let sites = crate::aarch64::place_bounded_buffer::append_bounded_buffer_source_to_x16(
        &mut bytes, source,
    )?;
    Ok((bytes, sites))
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_indexed_bounded_buffer_source_append(
    descriptor_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    source: &omega_target_operations::Place,
) -> Result<
    (
        Vec<u8>,
        crate::aarch64::place_bounded_buffer::BoundedBufferPlaceSites,
    ),
    Diagnostic,
> {
    let mut bytes = Vec::new();
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
    let sites = crate::aarch64::place_bounded_buffer::append_bounded_buffer_source_to_x16(
        &mut bytes, source,
    )?;
    Ok((bytes, sites))
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_bounded_buffer_source_append(
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    source: &omega_target_operations::Place,
) -> Result<
    (
        Vec<u8>,
        crate::aarch64::place_bounded_buffer::BoundedBufferPlaceSites,
    ),
    Diagnostic,
> {
    encode_runtime_frame_base_indexed_bounded_buffer_source_append_with_index_region(
        base_byte_offset,
        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        source,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_frame_base_indexed_bounded_buffer_source_append_with_index_region(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    source: &omega_target_operations::Place,
) -> Result<
    (
        Vec<u8>,
        crate::aarch64::place_bounded_buffer::BoundedBufferPlaceSites,
    ),
    Diagnostic,
> {
    let mut bytes = Vec::new();
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
    let sites = crate::aarch64::place_bounded_buffer::append_bounded_buffer_source_to_x16(
        &mut bytes, source,
    )?;
    Ok((bytes, sites))
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_indexed_bounded_buffer_source_append(
    base_byte_offset: usize,
    index_region: omega_target_operations::RuntimeStorageRegion,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    source: &omega_target_operations::Place,
) -> Result<
    (
        Vec<u8>,
        crate::aarch64::place_bounded_buffer::BoundedBufferPlaceSites,
    ),
    Diagnostic,
> {
    let mut bytes = Vec::new();
    append_runtime_machine_index_target_address(
        &mut bytes,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )?;
    let sites = crate::aarch64::place_bounded_buffer::append_bounded_buffer_source_to_x16(
        &mut bytes, source,
    )?;
    Ok((bytes, sites))
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_machine_double_indexed_bounded_buffer_source_append(
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
    source: &omega_target_operations::Place,
) -> Result<
    (
        Vec<u8>,
        crate::aarch64::place_bounded_buffer::BoundedBufferPlaceSites,
    ),
    Diagnostic,
> {
    let mut bytes = Vec::new();
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
    let sites = crate::aarch64::place_bounded_buffer::append_bounded_buffer_source_to_x16(
        &mut bytes, source,
    )?;
    Ok((bytes, sites))
}

pub fn place_bounded_buffer_write_register_write_ceiling() -> RegisterSet {
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

pub const fn place_bounded_buffer_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

/// Append a string LITERAL onto an owned `[u8; N]` carrier at its running
/// length (a later concat segment, e.g. the trailing `" =="`). x16 = machine
/// storage base (the only relocation, at instruction start); x15 = running
/// length; x14 = byte cursor (`base + target + 8 + len`, advanced by
/// post-increment stores); the literal bytes are immediates. The new length
/// (`len + literal.len`) is stored last.
pub fn encode_runtime_machine_bounded_buffer_literal_append(
    target_byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_literal_append_width(
        target_byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = machine storage base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_load_x_from_x(15, 16, target_byte_offset)?); // x15 = running len
    append_add_x_constant(&mut bytes, 14, 16, target_byte_offset + 8, 13)?; // x14 = bytes base
    bytes.extend(encode_add_x_register(14, 14, 15)); // x14 = write cursor (bytes + len)
    for byte in literal {
        append_unsigned_immediate(&mut bytes, 17, u64::from(*byte));
        bytes.extend(encode_store_byte_w_post_increment(17, 14, 1)?);
    }
    bytes.extend(encode_add_x_immediate(15, 15, literal.len())?); // len += literal.len
    bytes.extend(encode_store_x_to_x(15, 16, target_byte_offset)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_literal_append_width(target_byte_offset, literal)
    );
    Ok(bytes)
}

/// Append a source carrier's content onto a target carrier (concat builder
/// source segment, after the first literal initialized the target). x16 = the
/// machine storage base (target; relocated at instruction start); a frame-local
/// source (`let`-local struct's carrier) adds a frame-base `adrp`+`add` pair for
/// x14 right after (relocated at the arch-aware +8 -- see the relocation
/// record). x15 = target running len, x13 = source len (consumed as the copy
/// counter), x12/x11 = source/target byte cursors, w17 = byte scratch. The new
/// length is stored BEFORE the copy loop, which decrements x13 to zero -- the
/// same must-precede rule as the x86_64 `rep movsb` encoder.
pub fn encode_runtime_machine_bounded_buffer_source_append(
    target_byte_offset: usize,
    source_byte_offset: usize,
    source_in_frame: bool,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_machine_bounded_buffer_source_append_width(
        target_byte_offset,
        source_byte_offset,
        source_in_frame,
    ));
    bytes.extend(encode_adrp_placeholder(16)); // x16 = machine storage base (reloc @ start)
    bytes.extend(encode_add_page_offset_placeholder(16));
    let source_base = if source_in_frame {
        bytes.extend(encode_adrp_placeholder(14)); // x14 = frame base (reloc @ +8)
        bytes.extend(encode_add_page_offset_placeholder(14));
        14
    } else {
        16
    };
    bytes.extend(encode_load_x_from_x(15, 16, target_byte_offset)?); // x15 = target len
    bytes.extend(encode_load_x_from_x(13, source_base, source_byte_offset)?); // x13 = source len
    append_add_x_constant(&mut bytes, 12, source_base, source_byte_offset + 8, 10)?; // x12 = src bytes
    append_add_x_constant(&mut bytes, 11, 16, target_byte_offset + 8, 10)?; // x11 = dst bytes base
    bytes.extend(encode_add_x_register(11, 11, 15)); // x11 = dst cursor (bytes + len)
    // new len = target_len + source_len -- MUST precede the loop, which
    // consumes x13 as it copies; computing it after would always add 0.
    bytes.extend(encode_add_x_register(15, 15, 13));
    bytes.extend(encode_store_x_to_x(15, 16, target_byte_offset)?);
    // Bounded byte copy:
    //   loop: cbz  x13, done   (+20: skip ldrb/strb/subs/b)
    //         ldrb w17, [x12], #1
    //         strb w17, [x11], #1
    //         subs x13, x13, #1
    //         b    loop        (-16)
    //   done:
    bytes.extend(encode_cbz_x(13, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(17, 12, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(17, 11, 1)?);
    bytes.extend(encode_subs_x_immediate(13, 13, 1)?);
    bytes.extend(encode_unconditional_branch(-16)?);
    debug_assert_eq!(
        bytes.len(),
        runtime_machine_bounded_buffer_source_append_width(
            target_byte_offset,
            source_byte_offset,
            source_in_frame
        )
    );
    Ok(bytes)
}
