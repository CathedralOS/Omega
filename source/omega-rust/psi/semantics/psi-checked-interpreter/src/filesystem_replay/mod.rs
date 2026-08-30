//! Typed filesystem replay vocabulary with responsibility-specific validation.
//!
//! The interpreter's broader replay grammar remains in the crate entrance for
//! now. New replay lanes belong behind named modules here instead of extending
//! that legacy monolith.

mod directories;
#[cfg(test)]
mod directory_tests;
mod duplicates;
#[cfg(test)]
mod handle_failure_tests;
mod handle_failures;
mod hard_links;
mod locks;
mod output_failures;
mod output_tree;
#[cfg(test)]
mod output_tree_tests;
mod source_directories;
#[cfg(test)]
mod source_directory_tests;
#[cfg(test)]
mod source_read_link_tests;
mod source_read_links;
mod symlinks;

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
pub(crate) use handle_failures::unknown_descriptor_failure_attempt_is_exact;
pub use handle_failures::{
    FilesystemInputUnknownDescriptorOperationReplayKind,
    FilesystemInputUnknownDescriptorOperationReplayRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
};
#[cfg(test)]
pub(crate) use handle_failures::{
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
    unknown_descriptor_seek_attempt, unknown_descriptor_seek_from_exact_attempt,
    unknown_descriptor_set_file_times_attempt,
    unknown_descriptor_set_file_times_from_exact_attempt,
    unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_from_exact_attempt,
};
pub use hard_links::{FilesystemOutputHardLinkReplayKind, FilesystemOutputHardLinkReplayRecord};
pub(crate) use hard_links::{output_hard_link_attempt, output_hard_link_record_from_attempt};
pub use locks::{FilesystemOutputLockReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS};
pub(crate) use locks::{
    output_lock_attempts, output_lock_record_from_attempts, validate_output_lock_replay,
};
pub use output_failures::{
    FilesystemInputOutputAbsentRemovesReplayRecord, FilesystemOutputAbsentRemoveReplayRecord,
    MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES,
};
pub(crate) use output_failures::{
    output_absent_remove_attempt, output_absent_remove_record_from_attempt,
};
pub(crate) use output_tree::validate_observed_output_tree_records;
pub use output_tree::{
    FilesystemInputOutputTreeReplayRecord, FilesystemOutputTreeEntryReplayRecord,
};
pub use source_directories::{
    FilesystemSourceDirectoryReadChainReplayRecord, FilesystemSourceDirectoryReadReplayRecord,
};
pub(crate) use source_directories::{
    source_directory_chain_attempts, source_directory_chain_is_exact,
};
pub use source_read_links::FilesystemSourceReadLinkReplayRecord;
pub(crate) use source_read_links::{source_read_link_attempt, source_read_link_attempt_is_exact};
pub use symlinks::{
    FilesystemOutputSymlinkReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES,
};
pub(crate) use symlinks::{output_symlink_attempt, output_symlink_record_from_attempt};
