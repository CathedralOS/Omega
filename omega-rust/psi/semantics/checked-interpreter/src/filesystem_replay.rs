//! Typed filesystem replay vocabulary with responsibility-specific validation.
//!
//! Source/output replay construction and exact retained-operation validation live here.
//!
//! `replay_records.rs` holds the typed records, `replay_validation.rs`
//! validates a replay. `output_stream.rs` follows descriptor lifetimes in the
//! original event order; `output_attempts.rs` checks each file's projected
//! operations. `source_attempts.rs` owns the bounded Source prefix.

#[cfg(test)]
mod descriptor_error_state_failure_tests;
mod descriptor_error_state_failures;
mod directories;
#[cfg(test)]
mod directory_tests;
mod duplicates;
#[cfg(test)]
mod handle_failure_tests;
mod handle_failures;
mod hard_links;
mod locks;
#[cfg(test)]
mod native_error_state_failure_tests;
mod native_error_state_failures;
#[cfg(test)]
mod native_mutation_failure_tests;
mod native_mutation_failures;
#[cfg(test)]
mod native_query_chain_tests;
mod native_query_chains;
mod open_at_failures;
mod output_attempts;
mod output_failures;
mod output_ownership;
mod output_stream;
mod output_tree;
#[cfg(test)]
mod output_tree_tests;
#[cfg(test)]
mod read_dir_failure_tests;
mod read_dir_failures;
#[cfg(test)]
mod record_tests;
mod replay_records;
mod replay_validation;
mod source_attempts;
mod source_directories;
#[cfg(test)]
mod source_directory_tests;
#[cfg(test)]
mod source_read_link_tests;
mod source_read_links;
mod source_write_refusals;
mod symlinks;
mod unlink_at_failures;

use crate::BuildIncludedSource;
use crate::EvaluationObservations;
use crate::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION;
use crate::FilesystemOperationAttempt;
pub use descriptor_error_state_failures::FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord;
pub(crate) use descriptor_error_state_failures::ordered_descriptor_error_state_attempt_is_replayed;
pub use directories::{
    FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE, FilesystemInputOutputDirectoryReplayRecord,
    FilesystemOutputDirectoryReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES,
    MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES,
};
pub(crate) use directories::{
    output_directory_attempt, source_attempts_use_root, validate_output_directory_records,
};
pub use duplicates::{
    FilesystemOutputDuplicateReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES,
};
pub(crate) use duplicates::{output_logical_handle_identities, validate_output_duplicate_replay};
pub use handle_failures::{
    FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord,
    FilesystemInputUnknownDescriptorOperationReplayKind,
    FilesystemInputUnknownDescriptorOperationReplayRecord,
    FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord,
    FilesystemInputUnknownDescriptorReadReplayKind,
    FilesystemInputUnknownDescriptorReadReplayRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    FilesystemInputUnknownDescriptorWriteReplayKind,
    FilesystemInputUnknownDescriptorWriteReplayRecord,
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord,
};
#[cfg(test)]
pub(crate) use handle_failures::{
    unknown_descriptor_get_osfhandle_attempt, unknown_descriptor_get_osfhandle_attempt_is_exact,
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
    unknown_descriptor_read_attempt, unknown_descriptor_read_file_metadata_attempt,
    unknown_descriptor_read_file_metadata_from_exact_attempt,
    unknown_descriptor_read_from_exact_attempt, unknown_descriptor_seek_attempt,
    unknown_descriptor_seek_from_exact_attempt, unknown_descriptor_set_file_times_attempt,
    unknown_descriptor_set_file_times_from_exact_attempt, unknown_descriptor_write_attempt,
    unknown_descriptor_write_from_exact_attempt, unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_from_exact_attempt,
    unknown_native_handle_close_handle_attempt,
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_from_exact_attempt,
};
pub(crate) use hard_links::output_hard_link_attempt;
pub use hard_links::{FilesystemOutputHardLinkReplayKind, FilesystemOutputHardLinkReplayRecord};
pub(crate) use locks::validate_output_lock_replay;
pub use locks::{FilesystemOutputLockReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS};
pub use native_error_state_failures::FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord;
pub(crate) use native_error_state_failures::ordered_native_error_state_attempt_is_replayed;
pub use native_mutation_failures::{
    FilesystemInputUnknownNativeHandleMutationReplayKind,
    FilesystemInputUnknownNativeHandleMutationReplayRecord,
};
#[cfg(test)]
pub(crate) use native_query_chains::source_native_handle_query_chain_is_exact;
pub use native_query_chains::{
    FilesystemNativeHandleErrorObservationReplayRecord,
    FilesystemNativeHandleFinalPathQueryReplayRecord,
    FilesystemNativeHandleQueryOperationReplayRecord,
    FilesystemSourceNativeHandleQueryChainReplayRecord,
};
pub use open_at_failures::FilesystemInputUnknownDescriptorOpenAtReplayRecord;
#[cfg(test)]
pub(crate) use open_at_failures::{
    unknown_descriptor_open_at_attempt, unknown_descriptor_open_at_attempt_is_exact,
    unknown_descriptor_open_at_from_exact_attempt,
};
pub(crate) use output_attempts::{filesystem_output_attempt_tag, output_file_attempts};
pub(crate) use output_failures::output_absent_remove_attempt;
pub use output_failures::{
    FilesystemInputOutputAbsentRemovesReplayRecord, FilesystemOutputAbsentRemoveKind,
    FilesystemOutputAbsentRemoveReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES,
};
pub use output_ownership::FilesystemOutputChangeFileOwnerReplayRecord;
use output_stream::output_tree_from_attempts;
pub(crate) use output_tree::validate_observed_output_tree_records;
pub use output_tree::{
    FilesystemInputOutputTreeReplayRecord, FilesystemOutputTreeEntryReplayRecord,
};
pub use read_dir_failures::FilesystemInputUnknownDescriptorReadDirReplayRecord;
#[cfg(test)]
pub(crate) use read_dir_failures::{
    unknown_descriptor_read_dir_attempt, unknown_descriptor_read_dir_attempt_is_exact,
    unknown_descriptor_read_dir_from_exact_attempt,
};
pub(crate) use replay_records::output_file_attempt_count;
pub use replay_records::{
    FilesystemInputOutputReplayRecord, FilesystemOutputFileOperationReplayRecord,
    FilesystemOutputFileReplayRecord, FilesystemOutputWriteReplayKind,
    FilesystemOutputWriteReplayRecord, FilesystemReplayReadKind, FilesystemReplayReadRecord,
    FilesystemSourceDescriptorMetadataReplayRecord, FilesystemSourceInputReplayEventRecord,
    FilesystemSourceInputReplayRecord, FilesystemSourcePathMetadataReplayRecord,
    FilesystemSourceReadChainReplayRecord,
};
pub(crate) use replay_validation::{
    output_absent_remove_attempt_is_exact, source_write_refusal_attempt_is_exact,
    unknown_descriptor_bad_descriptor_failure_attempt_is_exact,
    unknown_input_handle_failure_attempt_is_exact, validate_filesystem_replay_size,
    validate_output_absent_remove_attempts, validate_output_replay_extents,
    validate_output_time_replay_retention, validate_source_input_attempts,
};
pub(crate) use source_attempts::{
    source_attempts_overlap_output, source_descriptor_close_attempt,
    source_descriptor_open_attempt, source_input_record_attempts,
};
#[cfg(test)]
pub(crate) use source_directories::source_directory_chain_is_exact;
pub use source_directories::{
    FilesystemSourceDirectoryReadChainReplayRecord, FilesystemSourceDirectoryReadReplayRecord,
};
pub use source_read_links::FilesystemSourceReadLinkReplayRecord;
#[cfg(test)]
pub(crate) use source_read_links::source_read_link_attempt_is_exact;
pub use source_write_refusals::{
    FilesystemSourceWriteRefusalReplayKind, FilesystemSourceWriteRefusalReplayRecord,
};
pub(crate) use source_write_refusals::{
    source_write_refusal_attempt, source_write_refusal_record_from_attempt,
};
pub(crate) use symlinks::output_symlink_attempt;
pub use symlinks::{
    FilesystemOutputSymlinkReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES,
};
pub use unlink_at_failures::FilesystemInputUnknownDescriptorUnlinkAtReplayRecord;
#[cfg(test)]
pub(crate) use unlink_at_failures::{
    unknown_descriptor_unlink_at_attempt, unknown_descriptor_unlink_at_attempt_is_exact,
    unknown_descriptor_unlink_at_from_exact_attempt,
};

/// Opaque, compiler-produced operation record for bounded filesystem replay.
/// Source events may be followed by an ordered parent-before-child Output tree
/// of directories, complete regular-file chains, symbolic links, and hard-link
/// names, by a closed failure-only Output-operation sequence, or by one closed
/// deterministic handle failure. File chains admit only their explicitly
/// validated descriptor operations, and generated-source handoffs retain exact
/// authored order.
/// The record is replay evidence; the compiler separately establishes receipt
/// strength by reproducing the build and matching sponsored staged-tree custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReplay {
    pub(crate) attempts: std::sync::Arc<[FilesystemOperationAttempt]>,
    pub(crate) expected_included_sources: std::sync::Arc<[BuildIncludedSource]>,
}

/// Ordinary non-executable create mode admitted by the first Output replay
/// rung (`0o666`, represented in Omega source as decimal `438`).
pub const FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE: i32 = 438;

pub const MAX_INCLUDED_BUILD_SOURCES: usize = 256;

pub const MAX_FILESYSTEM_REPLAY_RETAINED_BYTES: usize = 16 * 1024 * 1024;
// Cloning filesystem operation attempts has a deterministic availability
// limit. Fixed row weights are the current canonical row-width upper bounds;
// variable payload bytes contribute one unit each. This is deliberately not a
// second encoder, a Rust-layout measurement, or package evidence.

impl FilesystemReplay {
    pub(crate) fn executes_replay_attempt(&self, attempt_index: usize) -> bool {
        if self
            .attempts
            .get(attempt_index)
            .is_some_and(source_write_refusal_attempt_is_exact)
        {
            // This is a compiler-owned grant-policy result, not an Output
            // operation. Replay verifies its prepared Source coordinate and
            // injects the exact refusal without granting a virtual write.
            return false;
        }
        if self
            .attempts
            .get(attempt_index)
            .is_some_and(unknown_input_handle_failure_attempt_is_exact)
            || ordered_native_error_state_attempt_is_replayed(&self.attempts, attempt_index)
            || ordered_descriptor_error_state_attempt_is_replayed(&self.attempts, attempt_index)
        {
            return true;
        }
        self.attempts
            .iter()
            .position(|attempt| filesystem_output_attempt_tag(attempt.operation_tag()))
            .is_some_and(|output_start| attempt_index >= output_start)
    }

    /// Whether this replay contains any Output-rooted operation, including a
    /// failure-only sequence that leaves no final staged-tree entry.
    pub fn has_output_attempts(&self) -> bool {
        self.attempts.iter().any(|attempt| {
            !source_write_refusal_attempt_is_exact(attempt)
                && filesystem_output_attempt_tag(attempt.operation_tag())
        })
    }

    pub fn from_source_input_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        validate_source_input_attempts(attempts)?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    pub fn attempts(&self) -> &[FilesystemOperationAttempt] {
        &self.attempts
    }

    /// Reconstruct the typed Output files retained by this replay. Source-only
    /// records return an empty vector. Public constructors ensure every present
    /// file is exact, distinct, and ordered as authored.
    pub fn output_files(&self) -> Vec<FilesystemOutputFileReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(_)
                | FilesystemOutputTreeEntryReplayRecord::HardLink(_)
                | FilesystemOutputTreeEntryReplayRecord::Symlink(_) => None,
                FilesystemOutputTreeEntryReplayRecord::File(file) => Some(file),
            })
            .collect()
    }

    /// Reconstruct all exact Output entries in authored operation order.
    pub fn output_entries(&self) -> Vec<FilesystemOutputTreeEntryReplayRecord> {
        let Some(output_start) = self.attempts.iter().position(|attempt| {
            !source_write_refusal_attempt_is_exact(attempt)
                && filesystem_output_attempt_tag(attempt.operation_tag())
        }) else {
            return Vec::new();
        };
        if self.attempts[output_start..]
            .iter()
            .all(output_absent_remove_attempt_is_exact)
        {
            return Vec::new();
        }
        output_tree_from_attempts(&self.attempts[output_start..])
            .expect("validated filesystem replay retains exact Output entries")
            .entries
    }

    /// Reconstruct the exact ordered Output directories retained by this
    /// replay. File-only and source-only records return an empty vector.
    pub fn output_directories(&self) -> Vec<FilesystemOutputDirectoryReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(directory) => Some(directory),
                FilesystemOutputTreeEntryReplayRecord::File(_)
                | FilesystemOutputTreeEntryReplayRecord::HardLink(_)
                | FilesystemOutputTreeEntryReplayRecord::Symlink(_) => None,
            })
            .collect()
    }

    /// Reconstruct exact Output symlinks in authored operation order.
    pub fn output_symlinks(&self) -> Vec<FilesystemOutputSymlinkReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(_)
                | FilesystemOutputTreeEntryReplayRecord::File(_)
                | FilesystemOutputTreeEntryReplayRecord::HardLink(_) => None,
                FilesystemOutputTreeEntryReplayRecord::Symlink(symlink) => Some(symlink),
            })
            .collect()
    }

    /// Reconstruct exact Output hard links in authored operation order.
    pub fn output_hard_links(&self) -> Vec<FilesystemOutputHardLinkReplayRecord> {
        self.output_entries()
            .into_iter()
            .filter_map(|entry| match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(_)
                | FilesystemOutputTreeEntryReplayRecord::File(_)
                | FilesystemOutputTreeEntryReplayRecord::Symlink(_) => None,
                FilesystemOutputTreeEntryReplayRecord::HardLink(hard_link) => Some(hard_link),
            })
            .collect()
    }

    /// Generated-source coordinates expected during Output replay, in exact
    /// authored handoff order and with their filesystem-attempt ordinals.
    pub fn expected_included_sources(&self) -> &[BuildIncludedSource] {
        &self.expected_included_sources
    }

    pub fn from_source_input_record(
        record: FilesystemSourceInputReplayRecord,
    ) -> Result<Self, String> {
        let attempts = source_input_record_attempts(record);
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate the exact compiler-policy denial of one attempted create
    /// through a compiler-issued Source root.
    pub fn from_source_write_refusal_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        if !observations.build_included_sources().is_empty() {
            return Err(
                "filesystem replay refused Source write cannot hand off generated sources"
                    .to_owned(),
            );
        }
        if !observations.build_log().is_empty() {
            return Err(
                "filesystem replay refused Source write cannot carry BuildLog output".to_owned(),
            );
        }
        let [attempt] = observations.filesystem_operation_attempts() else {
            return Err("filesystem replay requires exactly one refused Source write".to_owned());
        };
        source_write_refusal_record_from_attempt(attempt)?;
        validate_filesystem_replay_size(std::slice::from_ref(attempt))?;
        Ok(Self {
            attempts: std::sync::Arc::from([attempt.clone()]),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Construct the exact refused Source-write replay from typed compiler
    /// coordinates.
    pub fn from_source_write_refusal_record(
        record: FilesystemSourceWriteRefusalReplayRecord,
    ) -> Result<Self, String> {
        let attempt = source_write_refusal_attempt(record);
        validate_filesystem_replay_size(std::slice::from_ref(&attempt))?;
        Ok(Self {
            attempts: std::sync::Arc::from([attempt]),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate an optional observed Source-input prefix followed by one or
    /// more exact Output entries, plus an exact ordered subset of explicit
    /// generated-source handoffs. A present Source prefix retains the same
    /// closed validation grammar as a source-bearing replay.
    pub fn from_input_output_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        Self::validate_input_output_attempts(attempts, observations.build_included_sources())?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: observations.build_included_sources().to_vec().into(),
        })
    }

    /// Admit a complete chronological Source/Output stream, including descriptor
    /// lifetimes, exact operation lanes, retained-byte bounds and source handoffs.
    /// Wire recovery and observed builds share this check; per-file projections
    /// never determine execution order.
    pub fn from_input_output_attempts(
        attempts: Vec<FilesystemOperationAttempt>,
        included_sources: Vec<BuildIncludedSource>,
    ) -> Result<Self, String> {
        Self::validate_input_output_attempts(&attempts, &included_sources)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: included_sources.into(),
        })
    }

    fn validate_input_output_attempts(
        attempts: &[FilesystemOperationAttempt],
        included_sources: &[BuildIncludedSource],
    ) -> Result<(), String> {
        validate_filesystem_replay_size(attempts)?;
        let output_start = attempts
            .iter()
            .position(|attempt| {
                !source_write_refusal_attempt_is_exact(attempt)
                    && filesystem_output_attempt_tag(attempt.operation_tag())
            })
            .ok_or_else(|| {
                "bounded filesystem replay requires one or more exact Output operations".to_owned()
            })?;
        if output_start > 0 {
            validate_source_input_attempts(&attempts[..output_start])?;
        }
        if attempts[output_start..]
            .iter()
            .all(output_absent_remove_attempt_is_exact)
        {
            validate_output_absent_remove_attempts(
                &attempts[..output_start],
                &attempts[output_start..],
                included_sources,
            )?;
            return Ok(());
        }
        let output = output_tree_from_attempts(&attempts[output_start..])?;
        validate_observed_output_tree_records(
            &attempts[..output_start],
            &output,
            attempts.len() - output_start,
            included_sources,
        )
    }

    /// Construct the closed optional-Source plus failure-only Output rung from
    /// typed compiler-owned coordinates.
    pub fn from_input_output_absent_removes_record(
        record: FilesystemInputOutputAbsentRemovesReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, absent_removes) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        let output_start = attempts.len();
        attempts.extend(absent_removes.into_iter().map(output_absent_remove_attempt));
        validate_filesystem_replay_size(&attempts)?;
        validate_output_absent_remove_attempts(
            &attempts[..output_start],
            &attempts[output_start..],
            &[],
        )?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Construct the same bounded grammar from already typed records.
    pub fn from_input_output_record(
        record: FilesystemInputOutputReplayRecord,
    ) -> Result<Self, String> {
        validate_output_duplicate_replay(&record.output_files)?;
        validate_output_time_replay_retention(&record.output_files)?;
        validate_output_replay_extents(&record.output_files)?;
        let mut attempts = source_input_record_attempts(record.source_input);
        for output in record.output_files {
            attempts.extend(output_file_attempts(output));
        }
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: record.expected_included_sources.into(),
        })
    }

    /// Construct the bounded optional-Source-input plus ordered Output-tree
    /// grammar from typed compiler-owned records. Directory and complete file
    /// entries retain their authored order.
    pub fn from_input_output_tree_record(
        record: FilesystemInputOutputTreeReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, output_entries, expected_included_sources) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        attempts.extend(
            output_entries
                .into_iter()
                .flat_map(FilesystemOutputTreeEntryReplayRecord::into_attempts),
        );
        Self::from_input_output_attempts(attempts, expected_included_sources)
    }

    /// Construct the bounded Source-input plus ordered empty Output-directory
    /// tree grammar from typed compiler-owned records.
    pub fn from_input_output_directory_record(
        record: FilesystemInputOutputDirectoryReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, output_directories) = record.into_parts();
        let mut attempts = source_input_record_attempts(source_input);
        attempts.extend(output_directories.into_iter().map(output_directory_attempt));
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}
