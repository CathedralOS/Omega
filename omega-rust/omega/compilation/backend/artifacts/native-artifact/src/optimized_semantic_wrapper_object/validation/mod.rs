//! Optimizer module role: executable entrance.

mod entry_contract;
mod provider_continuation;

pub use entry_contract::{ receiver_layout};
pub(crate) use entry_contract::{bind_semantic_contract, validate_entry_shape};
pub(crate) use provider_continuation::validate_installed_program_storage_continuation_evidence;
