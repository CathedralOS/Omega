//! Optimizer module role: executable entrance.

mod entry_contract;

pub use entry_contract::{ receiver_layout};
pub(crate) use entry_contract::{bind_semantic_contract, validate_entry_shape};
