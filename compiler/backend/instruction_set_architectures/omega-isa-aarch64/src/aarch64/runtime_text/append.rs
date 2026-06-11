use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    append_add_x_constant, append_unsigned_immediate_padded, encode_add_page_offset_placeholder,
    encode_add_x_register, encode_adrp_placeholder, encode_cbz_x,
    encode_conditional_branch_not_equal, encode_load_byte_w_post_increment, encode_load_x_from_x,
    encode_move_x_register, encode_movz_w, encode_store_byte_w_post_increment, encode_store_x_to_x,
    encode_subs_x_immediate,
};
use super::super::widths::{
    runtime_text_buffer_materialize_to_runtime_frame_indexed_width,
    runtime_text_buffer_materialize_to_runtime_pointee_width,
    runtime_text_buffer_materialize_width,
    runtime_text_literal_append_to_runtime_frame_indexed_width,
    runtime_text_literal_append_to_runtime_pointee_width, runtime_text_literal_append_width,
    runtime_text_stored_place_append_to_runtime_frame_indexed_width,
    runtime_text_stored_place_append_to_runtime_pointee_width,
    runtime_text_stored_place_append_width, runtime_text_stored_suffix_append_width,
};

pub fn encode_runtime_text_stored_suffix_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_suffix_append_width(
        buffer_offset,
        source_offset,
        target_offset,
        length_delta,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 26, 17, source_offset, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 17, source_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(23, 19));
    append_add_x_constant(&mut bytes, 22, 16, buffer_offset, 15)?;

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_store_x_to_x_offset(&mut bytes, 16, 17, target_offset, 15)?;
    append_add_x_constant(&mut bytes, 23, 23, length_delta, 15)?;
    append_store_x_to_x_offset(&mut bytes, 23, 17, target_offset + 8, 15)?;
    Ok(bytes)
}

pub fn encode_runtime_text_stored_place_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_place_append_width(
        source_offset,
        target_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 22, 17, target_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(24, 22));
    bytes.extend(encode_add_x_register(22, 16, 22));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_load_x_from_x_offset(&mut bytes, 26, 20, source_offset, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 20, source_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_add_x_register(24, 24, 23));
    append_store_x_to_x_offset(&mut bytes, 16, 17, target_offset, 15)?;
    append_store_x_to_x_offset(&mut bytes, 24, 17, target_offset + 8, 15)?;
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
    buffer_offset: usize,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_text_stored_place_append_to_runtime_frame_indexed_width(
            source_offset,
            element_byte_size,
            field_byte_offset,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_load_x_from_x(22, 16, 8)?);
    bytes.extend(encode_move_x_register(24, 22));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_move_x_register(20, 17));
    bytes.extend(encode_add_x_register(22, 17, 22));
    bytes.extend(encode_adrp_placeholder(25));
    bytes.extend(encode_add_page_offset_placeholder(25));
    append_load_x_from_x_offset(&mut bytes, 26, 25, source_offset, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 25, source_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_add_x_register(24, 24, 23));
    bytes.extend(encode_store_x_to_x(20, 16, 0)?);
    bytes.extend(encode_store_x_to_x(24, 16, 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_stored_place_append_to_runtime_pointee(
    buffer_offset: usize,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_stored_place_append_to_runtime_pointee_width(
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 17, 17, pointer_byte_offset, 15)?;
    if field_byte_offset > 0 {
        append_add_x_constant(&mut bytes, 17, 17, field_byte_offset, 15)?;
    }
    append_load_x_from_x_offset(&mut bytes, 22, 17, 8, 15)?;
    bytes.extend(encode_move_x_register(24, 22));
    bytes.extend(encode_add_x_register(22, 16, 22));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_load_x_from_x_offset(&mut bytes, 26, 20, source_offset, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 20, source_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_add_x_register(24, 24, 23));
    append_store_x_to_x_offset(&mut bytes, 16, 17, 0, 15)?;
    append_store_x_to_x_offset(&mut bytes, 24, 17, 8, 15)?;
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_literal_append(
    buffer_offset: usize,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_width(target_offset, literal));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 22, 17, target_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(20, 16));
    bytes.extend(encode_add_x_register(16, 16, 22));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        let _ = byte_index;
        bytes.extend(encode_movz_w(26, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_post_increment(26, 16, 1)?);
    }

    append_store_x_to_x_offset(&mut bytes, 20, 17, target_offset, 15)?;
    append_add_x_constant(&mut bytes, 22, 22, literal.len(), 15)?;
    append_store_x_to_x_offset(&mut bytes, 22, 17, target_offset + 8, 15)?;
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_literal_append_to_runtime_pointee(
    buffer_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_pointee_width(
        pointer_byte_offset,
        field_byte_offset,
        literal,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 17, 17, pointer_byte_offset, 15)?;
    if field_byte_offset > 0 {
        append_add_x_constant(&mut bytes, 17, 17, field_byte_offset, 15)?;
    }
    append_load_x_from_x_offset(&mut bytes, 22, 17, 8, 15)?;
    bytes.extend(encode_move_x_register(20, 16));
    bytes.extend(encode_add_x_register(16, 16, 22));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        let _ = byte_index;
        bytes.extend(encode_movz_w(26, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_post_increment(26, 16, 1)?);
    }

    append_store_x_to_x_offset(&mut bytes, 20, 17, 0, 15)?;
    append_add_x_constant(&mut bytes, 22, 22, literal.len(), 15)?;
    append_store_x_to_x_offset(&mut bytes, 22, 17, 8, 15)?;
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    buffer_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_append_to_runtime_frame_indexed_width(
        element_byte_size,
        field_byte_offset,
        literal,
    ));
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_load_x_from_x(22, 16, 8)?);
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_move_x_register(20, 17));
    bytes.extend(encode_add_x_register(17, 17, 22));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        let _ = byte_index;
        bytes.extend(encode_movz_w(26, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_post_increment(26, 17, 1)?);
    }

    bytes.extend(encode_store_x_to_x(20, 16, 0)?);
    append_add_x_constant(&mut bytes, 22, 22, literal.len(), 15)?;
    bytes.extend(encode_store_x_to_x(22, 16, 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize(target_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_buffer_materialize_width(target_offset));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 26, 17, target_offset, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 17, target_offset + 8, 15)?;
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_move_x_register(22, 16));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    append_store_x_to_x_offset(&mut bytes, 16, 17, target_offset, 15)?;
    append_store_x_to_x_offset(&mut bytes, 23, 17, target_offset + 8, 15)?;
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_buffer_materialize_to_runtime_pointee_width(
        pointer_byte_offset,
        field_byte_offset,
    ));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    append_load_x_from_x_offset(&mut bytes, 17, 17, pointer_byte_offset, 15)?;
    if field_byte_offset > 0 {
        append_add_x_constant(&mut bytes, 17, 17, field_byte_offset, 15)?;
    }
    append_load_x_from_x_offset(&mut bytes, 26, 17, 0, 15)?;
    append_load_x_from_x_offset(&mut bytes, 19, 17, 8, 15)?;
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_move_x_register(22, 16));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    append_store_x_to_x_offset(&mut bytes, 16, 17, 0, 15)?;
    append_store_x_to_x_offset(&mut bytes, 23, 17, 8, 15)?;
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(
        runtime_text_buffer_materialize_to_runtime_frame_indexed_width(
            element_byte_size,
            field_byte_offset,
        ),
    );
    append_runtime_frame_index_target_address(
        &mut bytes,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_load_x_from_x(26, 16, 0)?);
    bytes.extend(encode_load_x_from_x(19, 16, 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_move_x_register(22, 17));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 26, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    bytes.extend(encode_store_x_to_x(23, 16, 8)?);
    Ok(bytes)
}

fn append_runtime_frame_index_target_address(
    bytes: &mut Vec<u8>,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<(), Diagnostic> {
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    append_fixed_width_load_x_from_x_offset(bytes, 16, 20, descriptor_offset, 15);
    append_fixed_width_load_x_from_x_offset(bytes, 17, 20, index_offset, 21);
    append_scale_x_register_by_constant(bytes, 26, 17, element_byte_size)?;
    bytes.extend(encode_add_x_register(16, 16, 26));
    append_add_constant_to_x_register(bytes, 16, field_byte_offset)?;
    Ok(())
}

fn append_scale_x_register_by_constant(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    source_register: u8,
    scale: usize,
) -> Result<(), Diagnostic> {
    if scale == 0 {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot scale indexed runtime text storage by zero",
        ));
    }

    bytes.extend(encode_movz_w(destination_register, 0));
    let working_register = 19u8;
    bytes.extend(encode_move_x_register(working_register, source_register));

    let highest_bit = usize::BITS - scale.leading_zeros();
    for bit_index in 0..highest_bit {
        if (scale >> bit_index) & 1 == 1 {
            bytes.extend(encode_add_x_register(
                destination_register,
                destination_register,
                working_register,
            ));
        }

        if bit_index + 1 < highest_bit {
            bytes.extend(encode_add_x_register(
                working_register,
                working_register,
                working_register,
            ));
        }
    }

    Ok(())
}

fn append_add_constant_to_x_register(
    bytes: &mut Vec<u8>,
    register: u8,
    value: usize,
) -> Result<(), Diagnostic> {
    let scratch_register = if register == 19 { 26 } else { 19 };
    append_add_x_constant(bytes, register, register, value, scratch_register)
}

fn append_load_x_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, 8) {
        bytes.extend(encode_load_x_from_x(
            destination_register,
            base_register,
            byte_offset,
        )?);
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        bytes.extend(encode_load_x_from_x(
            destination_register,
            scratch_register,
            0,
        )?);
    }

    Ok(())
}

fn append_store_x_to_x_offset(
    bytes: &mut Vec<u8>,
    source_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) -> Result<(), Diagnostic> {
    if data_offset_encodable(byte_offset, 8) {
        bytes.extend(encode_store_x_to_x(
            source_register,
            base_register,
            byte_offset,
        )?);
    } else {
        bytes.extend(encode_move_x_register(scratch_register, base_register));
        append_add_constant_to_x_register(bytes, scratch_register, byte_offset)?;
        bytes.extend(encode_store_x_to_x(source_register, scratch_register, 0)?);
    }

    Ok(())
}

fn append_fixed_width_load_x_from_x_offset(
    bytes: &mut Vec<u8>,
    destination_register: u8,
    base_register: u8,
    byte_offset: usize,
    scratch_register: u8,
) {
    append_unsigned_immediate_padded(bytes, scratch_register, byte_offset as u64);
    bytes.extend(encode_add_x_register(
        scratch_register,
        base_register,
        scratch_register,
    ));
    bytes.extend(
        encode_load_x_from_x(destination_register, scratch_register, 0)
            .expect("zero-offset x-register load should always encode"),
    );
}

fn data_offset_encodable(byte_offset: usize, byte_size: usize) -> bool {
    match byte_size {
        1 => byte_offset <= 4095,
        4 => byte_offset.is_multiple_of(4) && byte_offset / 4 <= 4095,
        8 => byte_offset.is_multiple_of(8) && byte_offset / 8 <= 4095,
        _ => false,
    }
}
