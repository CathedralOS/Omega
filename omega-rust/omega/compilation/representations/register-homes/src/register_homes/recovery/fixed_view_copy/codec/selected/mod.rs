//! Optimizer module role: executable entrance. Selected-plan payload coordination inside the versioned artifact envelope.
//!
//! One ordinary function graph; structural contracts are instruction-keyed data.

mod block;
mod function;
mod instruction;
mod payload;
mod provenance;
mod register;
mod scalar;
mod structural;

pub(super) use self::payload::{decode as decode_selected_plan, encode as encode_selected_plan};
pub(super) use self::scalar::{decode as decode_ordinary_plan, encode as encode_ordinary_plan};

#[cfg(test)]
pub(super) use self::instruction::decode_kind;
