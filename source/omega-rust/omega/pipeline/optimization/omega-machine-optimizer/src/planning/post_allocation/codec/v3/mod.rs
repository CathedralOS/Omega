//! Optimizer module role: stage group. V3 machine-plan content vocabulary.
//!
//! Ordered plan decoding and its physical instruction/operand vocabulary are
//! the exact descendants of the sole current wire version.

mod decoding;
mod instruction;

pub(super) use decoding::decode_content;
