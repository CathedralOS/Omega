use omega_calling_conventions::{MachineRegister, MachineStateSet, RegisterSet};
use psi_diagnostics::Diagnostic;

use super::super::primitives::{
    encode_add_page_offset_placeholder, encode_adrp_placeholder, encode_movz_w,
    encode_store_byte_w17_to_x16,
};
use super::super::widths::runtime_text_literal_segment_write_width;

pub fn encode_runtime_text_literal_write(literal: &[u8]) -> Result<Vec<u8>, Diagnostic> {
    encode_runtime_text_literal_segment_write(0, literal)
}

pub fn runtime_text_literal_segment_write_register_writes() -> RegisterSet {
    RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)])
}

pub fn runtime_text_literal_segment_write_additional_machine_state() -> MachineStateSet {
    MachineStateSet::empty()
}

pub fn encode_runtime_text_literal_segment_write(
    byte_offset: usize,
    literal: &[u8],
) -> Result<Vec<u8>, Diagnostic> {
    let mut bytes = Vec::with_capacity(runtime_text_literal_segment_write_width(literal));
    bytes.extend(encode_adrp_placeholder(16));
    bytes.extend(encode_add_page_offset_placeholder(16));

    for (byte_index, byte) in literal.iter().enumerate() {
        bytes.extend(encode_movz_w(17, u16::from(*byte)));
        bytes.extend(encode_store_byte_w17_to_x16(byte_offset + byte_index)?);
    }

    Ok(bytes)
}
