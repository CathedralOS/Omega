//! Typed filesystem replay vocabulary with responsibility-specific validation.
//!
//! The interpreter's broader replay grammar remains in the crate entrance for
//! now. New replay lanes belong behind named modules here instead of extending
//! that legacy monolith.

mod directories;
#[cfg(test)]
mod directory_tests;
mod duplicates;
mod locks;
mod output_tree;
#[cfg(test)]
mod output_tree_tests;

pub use directories::{
    FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE, FilesystemInputOutputDirectoryReplayRecord,
    FilesystemOutputDirectoryReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES,
};
pub(crate) use directories::{
    output_directory_attempt, output_directory_record_from_attempt, source_attempts_use_root,
    validate_output_directory_records,
};
pub use duplicates::{
    FilesystemOutputDuplicateReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES,
};
pub(crate) use duplicates::{
    output_duplicate_attempts, output_duplicate_record_from_attempts,
    output_logical_handle_identities, validate_output_duplicate_replay,
};
pub use locks::{FilesystemOutputLockReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS};
pub(crate) use locks::{
    output_lock_attempts, output_lock_record_from_attempts, validate_output_lock_replay,
};
pub(crate) use output_tree::validate_observed_output_tree_records;
pub use output_tree::{
    FilesystemInputOutputTreeReplayRecord, FilesystemOutputTreeEntryReplayRecord,
};
