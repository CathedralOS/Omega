use omega_core::diagnostics::Diagnostic;

use super::super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_movz_w,
    encode_store_byte_w17_to_x16,
};

pub fn encode_runtime_text_literal_write(literal: &str) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_literal_segment_write(0, literal)
}

pub fn encode_runtime_text_literal_segment_write(
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = encode_adrp_placeholder(16);
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, byte) in literal.as_bytes().iter().enumerate() {
        bytes.extend(encode_movz_w(17, u16::from(*byte)));
        bytes.extend(encode_store_byte_w17_to_x16(byte_offset + byte_index)?);
    }

    Ok(bytes)
}
