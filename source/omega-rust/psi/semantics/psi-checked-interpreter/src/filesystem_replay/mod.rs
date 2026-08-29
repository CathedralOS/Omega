//! Typed filesystem replay vocabulary with responsibility-specific validation.
//!
//! The interpreter's broader replay grammar remains in the crate entrance for
//! now. New replay lanes belong behind named modules here instead of extending
//! that legacy monolith.

mod duplicates;

pub use duplicates::{
    FilesystemOutputDuplicateReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES,
};
pub(crate) use duplicates::{
    output_duplicate_attempts, output_duplicate_record_from_attempts,
    output_logical_handle_identities, validate_output_duplicate_replay,
};
