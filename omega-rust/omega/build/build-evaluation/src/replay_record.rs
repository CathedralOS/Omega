//!
//! `record.rs` holds the record, its limits and capture and recovery;
//! `rehydration.rs` turns a review-only record into exact shapes;
//! `attempt_codec.rs` encodes and decodes one attempt; `shape_validation.rs`
//! checks each decoded shape and `lane_selection.rs` names the lanes an
//! attempt kind may carry. The remaining modules each cover one replay
//! subject with its tests beside it.

#[cfg(test)]
mod absent_remove_tests;
mod attempt_codec;
#[cfg(test)]
mod descriptor_error_state_failure_tests;
mod descriptor_error_state_failures;
mod directories;
#[cfg(test)]
mod directory_tests;
mod duplicates;
mod exact_failure_rehydration;
#[cfg(test)]
mod handle_failure_tests;
mod handle_failures;
mod hard_links;
mod lane_selection;
#[cfg(test)]
mod lock_tests;
mod locks;
#[cfg(test)]
mod native_error_state_failure_tests;
mod native_error_state_failures;
#[cfg(test)]
mod native_mutation_failure_tests;
mod native_mutation_failures;
#[cfg(test)]
mod native_query_chain_tests;
#[cfg(test)]
mod output_only_tests;
mod output_ownership;
#[cfg(test)]
mod output_ownership_tests;
#[cfg(test)]
mod read_dir_failure_tests;
mod read_dir_failures;
#[cfg(test)]
mod read_link_tests;
mod read_links;
mod record;
mod rehydration;
mod shape_validation;
#[cfg(test)]
mod source_directory_tests;
#[cfg(test)]
mod source_write_refusal_tests;
mod symlinks;

pub use record::{
    BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits,
    ReviewOnlyBuildFilesystemReplayRecord, capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
pub use rehydration::rehydrate_review_only_build_filesystem_replay_record;
