//! Optimizer module role: executable entrance.

mod entry_contract;
mod provider_continuation;

pub use entry_contract::{
    bind_semantic_contract, receiver_layout, validate_entry_shape,
};
pub use provider_continuation::validate_installed_program_storage_continuation_evidence;


