//! Machine-plan content vocabulary shared by the supported wire versions.
//!
//! Ordered plan decoding and its physical instruction/operand vocabulary are
//! retained explicitly for decoding, without stage or validation authority.

mod decoding;
mod instruction;

pub(super) use decoding::decode_content;
