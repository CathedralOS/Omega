use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    encode_add_page_offset_placeholder, encode_add_x_immediate, encode_add_x_register,
    encode_adrp_placeholder, encode_cbz_x, encode_conditional_branch_not_equal,
    encode_load_byte_w_post_increment, encode_load_x_from_x, encode_move_x_register, encode_movz_w,
    encode_store_byte_w_post_increment, encode_store_byte_w_to_x, encode_store_x_to_x,
    encode_subs_x_immediate,
};

pub fn encode_runtime_text_stored_suffix_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(18, 17, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 17, source_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_add_x_immediate(22, 16, buffer_offset)?);

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_store_x_to_x(16, 17, target_offset)?);
    bytes.extend(encode_add_x_immediate(23, 23, length_delta)?);
    bytes.extend(encode_store_x_to_x(23, 17, target_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_text_stored_place_append(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(22, 17, target_offset + 8)?);
    bytes.extend(encode_move_x_register(24, 22));
    bytes.extend(encode_add_x_register(22, 16, 22));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(18, 20, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 20, source_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_add_x_register(24, 24, 23));
    bytes.extend(encode_store_x_to_x(16, 17, target_offset)?);
    bytes.extend(encode_store_x_to_x(24, 17, target_offset + 8)?);
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
    let mut bytes = encode_runtime_frame_index_target_address(
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
    bytes.extend(encode_load_x_from_x(18, 25, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 25, source_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
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
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(17, 17, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(17, 17, field_byte_offset)?);
    }
    bytes.extend(encode_load_x_from_x(22, 17, 8)?);
    bytes.extend(encode_move_x_register(24, 22));
    bytes.extend(encode_add_x_register(22, 16, 22));
    bytes.extend(encode_adrp_placeholder(20));
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(18, 20, source_offset)?);
    bytes.extend(encode_load_x_from_x(19, 20, source_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_add_x_register(24, 24, 23));
    bytes.extend(encode_store_x_to_x(16, 17, 0)?);
    bytes.extend(encode_store_x_to_x(24, 17, 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_literal_append(
    buffer_offset: usize,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(22, 17, target_offset + 8)?);
    bytes.extend(encode_move_x_register(20, 16));
    bytes.extend(encode_add_x_register(16, 16, 22));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_movz_w(18, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_to_x(18, 16, byte_index)?);
    }

    bytes.extend(encode_store_x_to_x(20, 17, target_offset)?);
    bytes.extend(encode_add_x_immediate(22, 22, literal.len())?);
    bytes.extend(encode_store_x_to_x(22, 17, target_offset + 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_literal_append_to_runtime_pointee(
    buffer_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(17, 17, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(17, 17, field_byte_offset)?);
    }
    bytes.extend(encode_load_x_from_x(22, 17, 8)?);
    bytes.extend(encode_move_x_register(20, 16));
    bytes.extend(encode_add_x_register(16, 16, 22));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_movz_w(18, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_to_x(18, 16, byte_index)?);
    }

    bytes.extend(encode_store_x_to_x(20, 17, 0)?);
    bytes.extend(encode_add_x_immediate(22, 22, literal.len())?);
    bytes.extend(encode_store_x_to_x(22, 17, 8)?);
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
    let mut bytes = encode_runtime_frame_index_target_address(
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
        bytes.extend(encode_movz_w(18, u16::from(*byte)));
        bytes.extend(encode_store_byte_w_to_x(18, 17, byte_index)?);
    }

    bytes.extend(encode_store_x_to_x(20, 16, 0)?);
    bytes.extend(encode_add_x_immediate(22, 22, literal.len())?);
    bytes.extend(encode_store_x_to_x(22, 16, 8)?);
    let _ = buffer_offset;
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize(target_offset: usize) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(18, 17, target_offset)?);
    bytes.extend(encode_load_x_from_x(19, 17, target_offset + 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_move_x_register(22, 16));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_store_x_to_x(16, 17, target_offset)?);
    bytes.extend(encode_store_x_to_x(23, 17, target_offset + 8)?);
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_pointee(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_load_x_from_x(17, 17, pointer_byte_offset)?);
    if field_byte_offset > 0 {
        bytes.extend(encode_add_x_immediate(17, 17, field_byte_offset)?);
    }
    bytes.extend(encode_load_x_from_x(18, 17, 0)?);
    bytes.extend(encode_load_x_from_x(19, 17, 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_move_x_register(22, 16));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_store_x_to_x(16, 17, 0)?);
    bytes.extend(encode_store_x_to_x(23, 17, 8)?);
    Ok(bytes)
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_runtime_frame_index_target_address(
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
    )?;
    bytes.extend(encode_load_x_from_x(18, 16, 0)?);
    bytes.extend(encode_load_x_from_x(19, 16, 8)?);
    bytes.extend(encode_move_x_register(23, 19));
    bytes.extend(encode_adrp_placeholder(17));
    bytes.extend(encode_add_page_offset_placeholder(17));
    bytes.extend(encode_move_x_register(22, 17));

    bytes.extend(encode_cbz_x(19, 20)?);
    bytes.extend(encode_load_byte_w_post_increment(21, 18, 1)?);
    bytes.extend(encode_store_byte_w_post_increment(21, 22, 1)?);
    bytes.extend(encode_subs_x_immediate(19, 19, 1)?);
    bytes.extend(encode_conditional_branch_not_equal(-12)?);

    bytes.extend(encode_store_x_to_x(17, 16, 0)?);
    bytes.extend(encode_store_x_to_x(23, 16, 8)?);
    Ok(bytes)
}

fn encode_runtime_frame_index_target_address(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(20);
    bytes.extend(encode_add_page_offset_placeholder(20));
    bytes.extend(encode_load_x_from_x(16, 20, descriptor_offset)?);
    bytes.extend(encode_load_x_from_x(17, 20, index_offset)?);
    bytes.extend(encode_scale_x_register_by_constant(18, 17, element_byte_size)?);
    bytes.extend(encode_add_x_register(16, 16, 18));
    bytes.extend(encode_add_constant_to_x_register(16, field_byte_offset)?);
    Ok(bytes)
}

fn encode_scale_x_register_by_constant(
    destination_register: u8,
    source_register: u8,
    scale: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if scale == 0 {
        return Err(Diagnostic::error(
            "AArch64 MVP encoder cannot scale indexed runtime text storage by zero",
        ));
    }

    let mut bytes = encode_movz_w(destination_register, 0);
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

    Ok(bytes)
}

fn encode_add_constant_to_x_register(
    register: u8,
    value: usize,
) -> Result<Vec<u8>, Diagnostic> {
    if value == 0 {
        return Ok(Vec::new());
    }
    if value <= 4095 {
        return encode_add_x_immediate(register, register, value);
    }

    let mut bytes = encode_movz_w(19, (value & 0xffff) as u16);
    bytes.extend(encode_add_x_register(register, register, 19));
    Ok(bytes)
}
