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
