//! Typed filesystem replay vocabulary with responsibility-specific validation.
//!
//! Source/output replay construction and exact retained-operation validation live here.

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
mod open_at_failures;
mod output_failures;
mod output_ownership;
mod output_tree;
#[cfg(test)]
mod output_tree_tests;
#[cfg(test)]
mod read_dir_failure_tests;
mod read_dir_failures;
mod source_directories;
#[cfg(test)]
mod source_directory_tests;
#[cfg(test)]
mod source_read_link_tests;
mod source_read_links;
mod source_write_refusals;
mod symlinks;
mod unlink_at_failures;

pub use descriptor_error_state_failures::FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord;
pub(crate) use descriptor_error_state_failures::ordered_descriptor_error_state_attempt_is_replayed;
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
pub use hard_links::{FilesystemOutputHardLinkReplayKind, FilesystemOutputHardLinkReplayRecord};
pub(crate) use hard_links::{output_hard_link_attempt, output_hard_link_record_from_attempt};
pub use locks::{FilesystemOutputLockReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS};
pub(crate) use locks::{
    output_lock_attempts, output_lock_record_from_attempts, validate_output_lock_replay,
};
pub use native_error_state_failures::FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord;
pub(crate) use native_error_state_failures::ordered_native_error_state_attempt_is_replayed;
pub use native_mutation_failures::{
    FilesystemInputUnknownNativeHandleMutationReplayKind,
    FilesystemInputUnknownNativeHandleMutationReplayRecord,
};
pub use open_at_failures::FilesystemInputUnknownDescriptorOpenAtReplayRecord;
#[cfg(test)]
pub(crate) use open_at_failures::{
    unknown_descriptor_open_at_attempt, unknown_descriptor_open_at_attempt_is_exact,
    unknown_descriptor_open_at_from_exact_attempt,
};
pub use output_failures::{
    FilesystemInputOutputAbsentRemovesReplayRecord, FilesystemOutputAbsentRemoveKind,
    FilesystemOutputAbsentRemoveReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES,
};
pub(crate) use output_failures::{
    output_absent_remove_attempt, output_absent_remove_record_from_attempt,
};
pub use output_ownership::FilesystemOutputChangeFileOwnerReplayRecord;
pub(crate) use output_ownership::{
    output_change_file_owner_attempt, output_change_file_owner_record_from_attempt,
};
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
pub use source_directories::{
    FilesystemSourceDirectoryReadChainReplayRecord, FilesystemSourceDirectoryReadReplayRecord,
};
pub(crate) use source_directories::{
    source_directory_chain_attempts, source_directory_chain_is_exact,
};
pub use source_read_links::FilesystemSourceReadLinkReplayRecord;
pub(crate) use source_read_links::{source_read_link_attempt, source_read_link_attempt_is_exact};
pub use source_write_refusals::{
    FilesystemSourceWriteRefusalReplayKind, FilesystemSourceWriteRefusalReplayRecord,
};
pub(crate) use source_write_refusals::{
    source_write_refusal_attempt, source_write_refusal_record_from_attempt,
};

pub(crate) fn source_write_refusal_attempt_is_exact(
    attempt: &crate::FilesystemOperationAttempt,
) -> bool {
    source_write_refusal_record_from_attempt(attempt).is_ok()
}
pub use symlinks::{
    FilesystemOutputSymlinkReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES,
};
pub(crate) use symlinks::{output_symlink_attempt, output_symlink_record_from_attempt};
pub use unlink_at_failures::FilesystemInputUnknownDescriptorUnlinkAtReplayRecord;
#[cfg(test)]
pub(crate) use unlink_at_failures::{
    unknown_descriptor_unlink_at_attempt, unknown_descriptor_unlink_at_attempt_is_exact,
    unknown_descriptor_unlink_at_from_exact_attempt,
};

pub(crate) fn unknown_input_handle_failure_attempt_is_exact(
    attempt: &crate::FilesystemOperationAttempt,
) -> bool {
    handle_failures::unknown_input_handle_failure_attempt_is_exact(attempt)
        || open_at_failures::unknown_descriptor_open_at_attempt_is_exact(attempt)
        || read_dir_failures::unknown_descriptor_read_dir_attempt_is_exact(attempt)
        || unlink_at_failures::unknown_descriptor_unlink_at_attempt_is_exact(attempt)
        || native_mutation_failures::unknown_native_handle_mutation_attempt_is_exact(attempt)
}

pub(crate) fn unknown_descriptor_bad_descriptor_failure_attempt_is_exact(
    attempt: &crate::FilesystemOperationAttempt,
) -> bool {
    unknown_input_handle_failure_attempt_is_exact(attempt)
        && matches!(
            attempt,
            crate::FilesystemOperationAttempt {
                outcome: Some(crate::FilesystemOperationAttemptOutcome::Returned {
                    post_error: 9,
                    ..
                }),
                logical_handle_inputs,
                ..
            } if matches!(
                logical_handle_inputs.as_slice(),
                [crate::FilesystemLogicalHandleInput {
                    kind: crate::FilesystemLogicalHandleKind::Descriptor,
                    resolution: crate::FilesystemLogicalHandleInputResolution::Unknown,
                    ..
                }]
            )
        )
}

use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_METADATA_API_CARRIER_BYTES,
    FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION, FilesystemAuthorizedPath, FilesystemByteOperand,
    FilesystemGrantAccess, FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemLogicalHandleOutput,
    FilesystemLogicalHandleOutputSource, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemObservedByteRegionKind, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand, FilesystemScalarOperandValue,
    filesystem_root_relative_path_is_canonical,
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
const MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT: usize = 16 * 1024 * 1024;
// Attempt = 16 fixed bytes + fifteen 8-byte lane counts + the 19-byte maximum
// logical-output row. The remaining weights are each row's fixed-width maximum
// in encode_attempt; byte-bearing lanes add their payloads below.
const FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT: usize = 155;
const FILESYSTEM_REPLAY_SCALAR_OPERAND_RETENTION_WEIGHT: usize = 10;
const FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_PATH_LIKE_OPERAND_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_ROOTED_PATH_RETENTION_WEIGHT: usize = 10;
const FILESYSTEM_REPLAY_RETURNED_PATH_RETENTION_WEIGHT: usize = 11;
const FILESYSTEM_REPLAY_OBSERVED_REGION_RETENTION_WEIGHT: usize = 18;
const FILESYSTEM_REPLAY_METADATA_RETENTION_WEIGHT: usize = 102;
const FILESYSTEM_REPLAY_MUTABLE_BYTE_RESOLUTION_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_MUTABLE_I64_RESOLUTION_RETENTION_WEIGHT: usize = 9;
const FILESYSTEM_REPLAY_MUTABLE_BYTE_RETENTION_WEIGHT: usize = 17;
const FILESYSTEM_REPLAY_MUTABLE_I64_RETENTION_WEIGHT: usize = 17;
const FILESYSTEM_REPLAY_AUTHORIZED_PATH_RETENTION_WEIGHT: usize = 11;
const FILESYSTEM_REPLAY_LOGICAL_INPUT_RETENTION_WEIGHT: usize = 11;
const FILESYSTEM_REPLAY_RETIRED_HANDLE_RETENTION_WEIGHT: usize = 8;
const FILESYSTEM_REPLAY_GRANT_REFUSAL_RETENTION_WEIGHT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemReplayReadKind {
    Sequential,
    Positioned { offset: i64 },
}

/// One typed read in the first replay rung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemReplayReadRecord {
    pub(crate) read_kind: FilesystemReplayReadKind,
    pub(crate) requested_count: u64,
    pub(crate) read_result: i64,
    pub(crate) read_post_error: i32,
    pub(crate) mutable_resolution: Vec<u8>,
    pub(crate) mutable_pre_state: Vec<u8>,
    pub(crate) mutable_post_state: Vec<u8>,
}

impl FilesystemReplayReadRecord {
    pub fn new(
        read_kind: FilesystemReplayReadKind,
        requested_count: u64,
        read_result: i64,
        read_post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
    ) -> Result<Self, String> {
        let read_length = usize::try_from(read_result)
            .map_err(|_| "filesystem replay read result must be nonnegative".to_owned())?;
        let requested_capacity = usize::try_from(requested_count)
            .map_err(|_| "filesystem replay request exceeds this host".to_owned())?;
        if matches!(read_kind, FilesystemReplayReadKind::Positioned { offset } if offset < 0)
            || mutable_resolution != mutable_pre_state
            || mutable_pre_state.len() != mutable_post_state.len()
            || requested_capacity > mutable_post_state.len()
            || read_length > requested_capacity
            || mutable_pre_state[read_length..] != mutable_post_state[read_length..]
        {
            return Err("filesystem replay mutable read carrier is inconsistent".to_owned());
        }
        Ok(Self {
            read_kind,
            requested_count,
            read_result,
            read_post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
        })
    }
}

/// One closed source-read chain reconstructed from canonical compiler custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceReadChainReplayRecord {
    pub(crate) source_root: FilesystemGrantRootIdentity,
    pub(crate) source_relative_path: Vec<u8>,
    pub(crate) logical_handle_identity: FilesystemLogicalHandleIdentity,
    pub(crate) open_post_error: i32,
    pub(crate) reads: Vec<FilesystemReplayReadRecord>,
    pub(crate) close_post_error: i32,
}

impl FilesystemSourceReadChainReplayRecord {
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        open_post_error: i32,
        reads: Vec<FilesystemReplayReadRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay logical identity must be nonzero".to_owned())?;
        if reads.is_empty() {
            return Err("filesystem replay requires at least one read".to_owned());
        }
        Ok(Self {
            source_root,
            source_relative_path,
            logical_handle_identity,
            open_post_error,
            reads,
            close_post_error,
        })
    }
}

/// One successful Source-rooted path metadata read reconstructed from
/// canonical compiler custody. The authored rooted input and the separately
/// authorized target both remain exact because following a symlink may make
/// them differ. Returned bytes are carried by the mutable post-state and
/// checked against `metadata` under the selected target layout during replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourcePathMetadataReplayRecord {
    pub(crate) kind: FilesystemMetadataObservationKind,
    pub(crate) source_root: FilesystemGrantRootIdentity,
    pub(crate) source_relative_path: Vec<u8>,
    pub(crate) authorized_root: FilesystemGrantRootIdentity,
    pub(crate) authorized_relative_path: Vec<u8>,
    pub(crate) post_error: i32,
    pub(crate) mutable_resolution: Vec<u8>,
    pub(crate) mutable_pre_state: Vec<u8>,
    pub(crate) mutable_post_state: Vec<u8>,
    pub(crate) metadata: FilesystemMetadataObservation,
}

impl FilesystemSourcePathMetadataReplayRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: FilesystemMetadataObservationKind,
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        authorized_root: FilesystemGrantRootIdentity,
        authorized_relative_path: Vec<u8>,
        post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
        metadata: FilesystemMetadataObservation,
    ) -> Result<Self, String> {
        if !matches!(
            kind,
            FilesystemMetadataObservationKind::FollowedPath
                | FilesystemMetadataObservationKind::UnfollowedFinalPath
        ) || metadata.kind() != kind
            || metadata.output_operand_ordinal() != 1
            || source_root != authorized_root
            || !filesystem_root_relative_path_is_canonical(&source_relative_path, false)
            || !filesystem_root_relative_path_is_canonical(&authorized_relative_path, true)
            || mutable_resolution != mutable_pre_state
            || mutable_pre_state.len() != mutable_post_state.len()
            || mutable_post_state.len() < FILESYSTEM_METADATA_API_CARRIER_BYTES
        {
            return Err("filesystem replay path metadata is inconsistent".to_owned());
        }
        Ok(Self {
            kind,
            source_root,
            source_relative_path,
            authorized_root,
            authorized_relative_path,
            post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
            metadata,
        })
    }
}

/// One successful Source-rooted descriptor metadata event. The descriptor is
/// created by the event's exact flags-zero open, observed once, and retired by
/// its exact close; it cannot be borrowed from or leaked into another event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceDescriptorMetadataReplayRecord {
    pub(crate) source_root: FilesystemGrantRootIdentity,
    pub(crate) source_relative_path: Vec<u8>,
    pub(crate) logical_handle_identity: FilesystemLogicalHandleIdentity,
    pub(crate) open_post_error: i32,
    pub(crate) metadata_post_error: i32,
    pub(crate) mutable_resolution: Vec<u8>,
    pub(crate) mutable_pre_state: Vec<u8>,
    pub(crate) mutable_post_state: Vec<u8>,
    pub(crate) metadata: FilesystemMetadataObservation,
    pub(crate) close_post_error: i32,
}

impl FilesystemSourceDescriptorMetadataReplayRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        open_post_error: i32,
        metadata_post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
        metadata: FilesystemMetadataObservation,
        close_post_error: i32,
    ) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay logical identity must be nonzero".to_owned())?;
        if !filesystem_root_relative_path_is_canonical(&source_relative_path, false)
            || metadata.kind() != FilesystemMetadataObservationKind::OpenDescriptor
            || metadata.output_operand_ordinal() != 1
            || mutable_resolution != mutable_pre_state
            || mutable_pre_state.len() != mutable_post_state.len()
            || mutable_post_state.len() < FILESYSTEM_METADATA_API_CARRIER_BYTES
        {
            return Err("filesystem replay descriptor metadata is inconsistent".to_owned());
        }
        Ok(Self {
            source_root,
            source_relative_path,
            logical_handle_identity,
            open_post_error,
            metadata_post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
            metadata,
            close_post_error,
        })
    }
}

/// One ordered source-input replay event. Descriptor-backed file reads and
/// descriptor metadata remain indivisible closed chains; `read_link` and path
/// metadata reads are independent events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemSourceInputReplayEventRecord {
    ReadChain(FilesystemSourceReadChainReplayRecord),
    DirectoryReadChain(FilesystemSourceDirectoryReadChainReplayRecord),
    ReadLink(FilesystemSourceReadLinkReplayRecord),
    DescriptorMetadata(FilesystemSourceDescriptorMetadataReplayRecord),
    PathMetadata(FilesystemSourcePathMetadataReplayRecord),
}

/// Typed source-input replay record reconstructed after canonical bytes cross
/// a process boundary. It grants no ambient host filesystem authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceInputReplayRecord {
    pub(crate) events: Vec<FilesystemSourceInputReplayEventRecord>,
}

impl FilesystemSourceInputReplayRecord {
    pub fn new(events: Vec<FilesystemSourceInputReplayEventRecord>) -> Result<Self, String> {
        if events.is_empty() {
            return Err("filesystem replay requires at least one source-input event".to_owned());
        }
        let mut identities = Vec::new();
        for event in &events {
            let identity = match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                    Some(chain.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                    Some(chain.logical_handle_identity())
                }
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => None,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                    Some(metadata.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => None,
            };
            let Some(identity) = identity else { continue };
            if identities.contains(&identity) {
                return Err(
                    "filesystem replay descriptor events must use distinct handles".to_owned(),
                );
            }
            identities.push(identity);
        }
        Ok(Self { events })
    }
}

/// Cursor behavior for one complete write within a freshly created Output file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOutputWriteReplayKind {
    Sequential,
    Positioned { offset: i64 },
}

/// One complete sequential or positioned write within a freshly created
/// Output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputWriteReplayRecord {
    pub(crate) kind: FilesystemOutputWriteReplayKind,
    pub(crate) bytes: Vec<u8>,
    pub(crate) result: i64,
    pub(crate) post_error: i32,
}

impl FilesystemOutputWriteReplayRecord {
    pub fn new(bytes: Vec<u8>, result: i64, post_error: i32) -> Result<Self, String> {
        Self::with_kind(
            FilesystemOutputWriteReplayKind::Sequential,
            bytes,
            result,
            post_error,
        )
    }

    pub fn positioned(
        offset: i64,
        bytes: Vec<u8>,
        result: i64,
        post_error: i32,
    ) -> Result<Self, String> {
        if offset < 0 {
            return Err(
                "filesystem replay positioned output offset must be nonnegative".to_owned(),
            );
        }
        Self::with_kind(
            FilesystemOutputWriteReplayKind::Positioned { offset },
            bytes,
            result,
            post_error,
        )
    }

    pub(crate) fn with_kind(
        kind: FilesystemOutputWriteReplayKind,
        bytes: Vec<u8>,
        result: i64,
        post_error: i32,
    ) -> Result<Self, String> {
        let full_result = i64::try_from(bytes.len())
            .map_err(|_| "filesystem replay output exceeds i64 write length".to_owned())?;
        if result != full_result {
            return Err(
                "filesystem replay output write must consume the complete immutable operand"
                    .to_owned(),
            );
        }
        Ok(Self {
            kind,
            bytes,
            result,
            post_error,
        })
    }

    pub const fn kind(&self) -> FilesystemOutputWriteReplayKind {
        self.kind
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn result(&self) -> i64 {
        self.result
    }

    pub const fn post_error(&self) -> i32 {
        self.post_error
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemOutputFileOperationReplayRecord {
    Write(FilesystemOutputWriteReplayRecord),
    Seek {
        offset: i64,
        whence: i32,
        result: i64,
    },
    SetLength {
        length: i64,
    },
    SetFilePermissions {
        mode: u32,
    },
    SetFileTimes {
        times: Vec<u8>,
    },
    Sync,
    SyncData,
    DuplicateAndClose(FilesystemOutputDuplicateReplayRecord),
    LockAndUnlock(FilesystemOutputLockReplayRecord),
    ChangeFileOwner(FilesystemOutputChangeFileOwnerReplayRecord),
}

/// One freshly created and closed Output file, optionally containing complete
/// sequential or positioned writes.
///
/// The file grammar is deliberately narrow: canonical `create` (tag 1), zero
/// or more admitted operations through that descriptor, successful
/// `duplicate`/immediate-`close` pairs, then canonical `close` (tag 8) of the
/// original. A compiler may reconstruct the
/// exact attempts from this record, but the record does not claim publication,
/// receipt strength, or custody of a staged tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputFileReplayRecord {
    pub(crate) output_root: FilesystemGrantRootIdentity,
    pub(crate) output_relative_path: Vec<u8>,
    pub(crate) logical_handle_identity: FilesystemLogicalHandleIdentity,
    pub(crate) create_post_error: i32,
    pub(crate) operations: Vec<FilesystemOutputFileOperationReplayRecord>,
    pub(crate) close_post_error: i32,
}

impl FilesystemOutputFileReplayRecord {
    pub fn empty(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        close_post_error: i32,
    ) -> Result<Self, String> {
        Self::with_writes(
            output_root,
            output_relative_path,
            logical_handle_identity,
            create_post_error,
            Vec::new(),
            close_post_error,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        write_bytes: Vec<u8>,
        write_result: i64,
        write_post_error: i32,
        close_post_error: i32,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&output_relative_path, false) {
            return Err("filesystem replay output path must be canonical and non-root".to_owned());
        }
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay output identity must be nonzero".to_owned())?;
        let write =
            FilesystemOutputWriteReplayRecord::new(write_bytes, write_result, write_post_error)?;
        Self::with_writes(
            output_root,
            output_relative_path,
            logical_handle_identity.get(),
            create_post_error,
            vec![write],
            close_post_error,
        )
    }

    pub fn with_writes(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        writes: Vec<FilesystemOutputWriteReplayRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        Self::with_operations(
            output_root,
            output_relative_path,
            logical_handle_identity,
            create_post_error,
            writes
                .into_iter()
                .map(FilesystemOutputFileOperationReplayRecord::Write)
                .collect(),
            close_post_error,
        )
    }

    pub fn with_operations(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        create_post_error: i32,
        operations: Vec<FilesystemOutputFileOperationReplayRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&output_relative_path, false) {
            return Err("filesystem replay output path must be canonical and non-root".to_owned());
        }
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay output identity must be nonzero".to_owned())?;
        let record = Self {
            output_root,
            output_relative_path,
            logical_handle_identity,
            create_post_error,
            operations,
            close_post_error,
        };
        record.replayed_extents()?;
        Ok(record)
    }

    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        self.output_root
    }

    pub fn output_relative_path(&self) -> &[u8] {
        &self.output_relative_path
    }

    pub const fn logical_handle_identity(&self) -> FilesystemLogicalHandleIdentity {
        self.logical_handle_identity
    }

    pub const fn create_mode(&self) -> i32 {
        FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE
    }

    pub const fn create_post_error(&self) -> i32 {
        self.create_post_error
    }

    pub fn operations(&self) -> &[FilesystemOutputFileOperationReplayRecord] {
        &self.operations
    }

    /// The exact final descriptor-scoped permission operand, when the build
    /// authored one. Absence deliberately remains distinct from an authored
    /// mode equal to the create default.
    pub fn replayed_file_permissions(&self) -> Option<u32> {
        self.operations.iter().rev().find_map(|operation| {
            if let FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode } =
                operation
            {
                Some(*mode)
            } else {
                None
            }
        })
    }

    /// The final modeled modification time from an authored descriptor-scoped
    /// time operation. The complete carrier remains in the operation record;
    /// this projection exists only to verify the final virtual namespace.
    pub(crate) fn replayed_file_modification_time(&self) -> Option<i64> {
        self.operations.iter().rev().find_map(|operation| {
            let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
            else {
                return None;
            };
            Some(i64::from_le_bytes(times[16..24].try_into().expect(
                "validated replay timespec carrier has modification seconds",
            )))
        })
    }

    /// Canonical staged-tree executable class derived from the final authored
    /// permission mode. A newly created file with no permission operation is
    /// ordinary regardless of the capture host's ambient umask.
    pub fn replayed_executable(&self) -> bool {
        self.replayed_file_permissions()
            .is_some_and(|mode| mode & 0o111 != 0)
    }

    pub fn replayed_bytes(&self) -> Result<Vec<u8>, String> {
        let (_, peak_extent) = self.replayed_extents()?;
        if peak_extent > MAX_FILESYSTEM_REPLAY_RETAINED_BYTES {
            return Err(format!(
                "filesystem replay Output exceeds its {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte extent ceiling"
            ));
        }
        let mut output = Vec::new();
        let mut cursor = 0usize;
        for operation in &self.operations {
            let FilesystemOutputFileOperationReplayRecord::Write(write) = operation else {
                if let FilesystemOutputFileOperationReplayRecord::Seek { result, .. } = operation {
                    cursor = usize::try_from(*result).map_err(|_| {
                        "filesystem replay Output seek result exceeds this host".to_owned()
                    })?;
                }
                if let FilesystemOutputFileOperationReplayRecord::SetLength { length } = operation {
                    let length = usize::try_from(*length).map_err(|_| {
                        "filesystem replay Output length exceeds this host".to_owned()
                    })?;
                    output
                        .try_reserve(length.saturating_sub(output.len()))
                        .map_err(|_| "filesystem replay output allocation failed".to_owned())?;
                    output.resize(length, 0);
                }
                continue;
            };
            let start = match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => cursor,
                FilesystemOutputWriteReplayKind::Positioned { offset } => usize::try_from(offset)
                    .map_err(|_| {
                    "filesystem replay positioned output offset exceeds this host".to_owned()
                })?,
            };
            let end = start
                .checked_add(write.bytes.len())
                .ok_or_else(|| "filesystem replay output extent overflowed".to_owned())?;
            if !write.bytes.is_empty() {
                if output.len() < end {
                    output
                        .try_reserve(end - output.len())
                        .map_err(|_| "filesystem replay output allocation failed".to_owned())?;
                    output.resize(end, 0);
                }
                output[start..end].copy_from_slice(&write.bytes);
            }
            if write.kind == FilesystemOutputWriteReplayKind::Sequential {
                cursor = end;
            }
        }
        Ok(output)
    }

    pub const fn close_post_error(&self) -> i32 {
        self.close_post_error
    }

    pub(crate) fn replayed_extents(&self) -> Result<(usize, usize), String> {
        let mut cursor = 0usize;
        let mut extent = 0usize;
        let mut peak_extent = 0usize;
        let mut duplicate_identities = Vec::new();
        for operation in &self.operations {
            if let FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate) =
                operation
            {
                let identity = duplicate.logical_handle_identity();
                if identity == self.logical_handle_identity
                    || duplicate_identities.contains(&identity)
                {
                    return Err("filesystem replay Output duplicate identity is reused".to_owned());
                }
                duplicate_identities.push(identity);
                if duplicate_identities.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES {
                    return Err(format!(
                        "filesystem replay Output duplicates exceed the {MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES}-descriptor ceiling"
                    ));
                }
                continue;
            }
            let FilesystemOutputFileOperationReplayRecord::Write(write) = operation else {
                if let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
                    && times.len() < 32
                {
                    return Err(
                        "filesystem replay Output set_file_times carrier is shorter than two timespec records"
                            .to_owned(),
                    );
                }
                if let FilesystemOutputFileOperationReplayRecord::Seek {
                    offset,
                    whence,
                    result,
                } = operation
                {
                    let base = match whence {
                        0 => 0i64,
                        1 => i64::try_from(cursor).map_err(|_| {
                            "filesystem replay Output cursor exceeds i64".to_owned()
                        })?,
                        2 => i64::try_from(extent).map_err(|_| {
                            "filesystem replay Output extent exceeds i64".to_owned()
                        })?,
                        _ => {
                            return Err(
                                "filesystem replay Output seek whence is invalid".to_owned()
                            );
                        }
                    };
                    let expected = base.checked_add(*offset).ok_or_else(|| {
                        "filesystem replay Output seek result overflowed".to_owned()
                    })?;
                    if expected < 0 || expected != *result {
                        return Err(
                            "filesystem replay Output seek result is inconsistent".to_owned()
                        );
                    }
                    cursor = usize::try_from(expected).map_err(|_| {
                        "filesystem replay Output seek result exceeds this host".to_owned()
                    })?;
                }
                if let FilesystemOutputFileOperationReplayRecord::SetLength { length } = operation {
                    extent = usize::try_from(*length).map_err(|_| {
                        "filesystem replay Output length must be nonnegative and fit this host"
                            .to_owned()
                    })?;
                    peak_extent = peak_extent.max(extent);
                }
                continue;
            };
            let start = match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => cursor,
                FilesystemOutputWriteReplayKind::Positioned { offset } => usize::try_from(offset)
                    .map_err(|_| {
                    "filesystem replay positioned output offset exceeds this host".to_owned()
                })?,
            };
            let end = start
                .checked_add(write.bytes.len())
                .ok_or_else(|| "filesystem replay output extent overflowed".to_owned())?;
            if !write.bytes.is_empty() {
                extent = extent.max(end);
                peak_extent = peak_extent.max(extent);
            }
            if write.kind == FilesystemOutputWriteReplayKind::Sequential {
                cursor = end;
            }
        }
        Ok((extent, peak_extent))
    }
}

pub(crate) fn output_file_operation_attempt_count(
    operation: &FilesystemOutputFileOperationReplayRecord,
) -> usize {
    match operation {
        FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_)
        | FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_) => 2,
        _ => 1,
    }
}

pub(crate) fn output_file_attempt_count(
    output: &FilesystemOutputFileReplayRecord,
) -> Option<usize> {
    output
        .operations
        .iter()
        .try_fold(2usize, |count, operation| {
            count.checked_add(output_file_operation_attempt_count(operation))
        })
}

/// Typed record for the bounded Source-input/Output-file replay grammar.
/// Source events are replayed first in their authored order, followed by the
/// output files. Generated-source handoffs retain exact call order and the
/// filesystem-attempt ordinal after which each file was published. Unselected
/// output files remain ordinary artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputOutputReplayRecord {
    pub(crate) source_input: FilesystemSourceInputReplayRecord,
    pub(crate) output_files: Vec<FilesystemOutputFileReplayRecord>,
    pub(crate) expected_included_sources: Vec<BuildIncludedSource>,
}

impl FilesystemInputOutputReplayRecord {
    pub fn new(
        source_input: FilesystemSourceInputReplayRecord,
        output_files: Vec<FilesystemOutputFileReplayRecord>,
        expected_included_sources: Vec<BuildIncludedSource>,
    ) -> Result<Self, String> {
        if output_files.is_empty() {
            return Err("filesystem replay requires at least one Output file".to_owned());
        }
        validate_output_duplicate_replay(&output_files)?;
        validate_output_lock_replay(&output_files)?;
        let source_attempt_count = source_input
            .events
            .iter()
            .try_fold(0usize, |count, event| {
                count.checked_add(match event {
                    FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                        chain.reads.len().checked_add(2)?
                    }
                    FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                        chain.attempt_count()?
                    }
                    FilesystemSourceInputReplayEventRecord::ReadLink(_) => 1,
                    FilesystemSourceInputReplayEventRecord::DescriptorMetadata(_) => 3,
                    FilesystemSourceInputReplayEventRecord::PathMetadata(_) => 1,
                })
            })
            .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
        let mut descriptor_identities = source_input
            .events
            .iter()
            .filter_map(|event| match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                    Some(chain.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                    Some(chain.logical_handle_identity())
                }
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => None,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                    Some(metadata.logical_handle_identity)
                }
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => None,
            })
            .collect::<Vec<_>>();
        for (ordinal, output) in output_files.iter().enumerate() {
            if output_files[..ordinal].iter().any(|prior| {
                (prior.output_root == output.output_root
                    && prior.output_relative_path == output.output_relative_path)
                    || prior.logical_handle_identity == output.logical_handle_identity
            }) {
                return Err(
                    "filesystem replay Output paths and descriptors must be distinct".to_owned(),
                );
            }
            if source_input.events.iter().any(|event| match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                    chain.source_root == output.output_root
                        || chain.logical_handle_identity == output.logical_handle_identity
                }
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                    chain.source_root() == output.output_root
                        || chain.logical_handle_identity() == output.logical_handle_identity
                }
                FilesystemSourceInputReplayEventRecord::ReadLink(read_link) => {
                    read_link.source_root() == output.output_root
                        || read_link.authorized_root() == output.output_root
                }
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                    metadata.source_root == output.output_root
                        || metadata.logical_handle_identity == output.logical_handle_identity
                }
                FilesystemSourceInputReplayEventRecord::PathMetadata(metadata) => {
                    metadata.source_root == output.output_root
                        || metadata.authorized_root == output.output_root
                }
            }) {
                return Err(
                    "filesystem replay Source and Output roots and descriptors must be distinct"
                        .to_owned(),
                );
            }
            for identity in output_logical_handle_identities(output) {
                if descriptor_identities.contains(&identity) {
                    return Err(
                        "filesystem replay Source and Output descriptors must be globally distinct"
                            .to_owned(),
                    );
                }
                descriptor_identities.push(identity);
            }
        }
        validate_expected_included_sources(
            &output_files,
            &expected_included_sources,
            source_attempt_count,
        )?;
        Ok(Self {
            source_input,
            output_files,
            expected_included_sources,
        })
    }

    pub const fn source_input(&self) -> &FilesystemSourceInputReplayRecord {
        &self.source_input
    }

    pub fn output_files(&self) -> &[FilesystemOutputFileReplayRecord] {
        &self.output_files
    }

    pub fn expected_included_sources(&self) -> &[BuildIncludedSource] {
        &self.expected_included_sources
    }
}

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
        output_tree_entries_from_attempts(&self.attempts[output_start..])
            .expect("validated filesystem replay retains exact Output entries")
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
                observations.build_included_sources(),
            )?;
            return Ok(Self {
                attempts: attempts.to_vec().into(),
                expected_included_sources: std::sync::Arc::from([]),
            });
        }
        let output_entries = output_tree_entries_from_attempts(&attempts[output_start..])?;
        validate_observed_output_tree_records(
            &attempts[..output_start],
            &output_entries,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: observations.build_included_sources().to_vec().into(),
        })
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
        for entry in output_entries {
            match entry {
                FilesystemOutputTreeEntryReplayRecord::Directory(directory) => {
                    attempts.push(output_directory_attempt(directory));
                }
                FilesystemOutputTreeEntryReplayRecord::File(file) => {
                    attempts.extend(output_file_attempts(file));
                }
                FilesystemOutputTreeEntryReplayRecord::HardLink(hard_link) => {
                    attempts.push(output_hard_link_attempt(hard_link));
                }
                FilesystemOutputTreeEntryReplayRecord::Symlink(symlink) => {
                    attempts.push(output_symlink_attempt(symlink));
                }
            }
        }
        validate_filesystem_replay_size(&attempts)?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: expected_included_sources.into(),
        })
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

pub(crate) fn validate_output_time_replay_retention(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    outputs
        .iter()
        .flat_map(|output| output.operations.iter())
        .try_fold(0usize, |retained, operation| {
            let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
            else {
                return Some(retained);
            };
            retained.checked_add(times.len().checked_mul(3)?)
        })
        .filter(|retained| *retained <= MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "filesystem replay Output time carriers exceed the {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte retained-evidence ceiling"
            )
        })
}

pub(crate) fn validate_output_replay_extents(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    outputs
        .iter()
        .try_fold(0usize, |total, output| {
            total
                .checked_add(output.replayed_extents()?.1)
                .filter(|extent| *extent <= MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
                .ok_or_else(|| {
                    format!(
                        "filesystem replay Output exceeds its {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte aggregate extent ceiling"
                    )
                })
        })
        .map(|_| ())
}

pub(crate) fn validate_expected_included_sources(
    outputs: &[FilesystemOutputFileReplayRecord],
    included_sources: &[BuildIncludedSource],
    source_attempt_count: usize,
) -> Result<(), String> {
    if included_sources.len() > MAX_INCLUDED_BUILD_SOURCES {
        return Err(format!(
            "filesystem replay exceeds its {MAX_INCLUDED_BUILD_SOURCES}-source handoff ceiling"
        ));
    }
    let total_attempt_count = outputs
        .iter()
        .try_fold(source_attempt_count, |count, output| {
            count.checked_add(output_file_attempt_count(output)?)
        })
        .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
    let mut previous_ordinal = source_attempt_count;
    for (handoff_index, included) in included_sources.iter().enumerate() {
        if included.filesystem_attempt_ordinal() < previous_ordinal {
            return Err(
                "filesystem replay included-source handoff ordinals must be nondecreasing"
                    .to_owned(),
            );
        }
        previous_ordinal = included.filesystem_attempt_ordinal();
        if included_sources[..handoff_index].iter().any(|prior| {
            prior.root() == included.root() && prior.relative_path() == included.relative_path()
        }) {
            return Err(
                "filesystem replay included-source handoff names one output more than once"
                    .to_owned(),
            );
        }
        let Some(output_index) = outputs.iter().position(|output| {
            output.output_root() == included.root()
                && output.output_relative_path() == included.relative_path()
        }) else {
            return Err(
                "filesystem replay included-source handoff has no matching output file".to_owned(),
            );
        };
        let earliest_ordinal = outputs[..=output_index]
            .iter()
            .try_fold(source_attempt_count, |count, output| {
                count.checked_add(output_file_attempt_count(output)?)
            })
            .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
        if included.filesystem_attempt_ordinal() < earliest_ordinal
            || included.filesystem_attempt_ordinal() > total_attempt_count
        {
            return Err(
                "filesystem replay included-source handoff must follow its exact Output close"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_source_input_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<(), String> {
    let mut cursor = 0;
    let mut event_count = 0;
    while cursor < attempts.len() {
        if filesystem_output_attempt_tag(attempts[cursor].operation_tag()) {
            break;
        }
        if attempts[cursor].operation_tag() == 21 {
            if !source_read_link_attempt_is_exact(&attempts[cursor]) {
                return Err(
                    "bounded filesystem replay source read-link event is inconsistent".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(attempts[cursor].operation_tag(), 38 | 40) {
            if !source_path_metadata_attempt_is_exact(&attempts[cursor]) {
                return Err("bounded filesystem replay source metadata is inconsistent".to_owned());
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if attempts[cursor].operation_tag() != 2 {
            return Err(
                "bounded filesystem replay requires ordered source-input events".to_owned(),
            );
        }
        let event_start = cursor;
        cursor += 1;
        if cursor < attempts.len() && attempts[cursor].operation_tag() == 39 {
            cursor += 1;
            if cursor == attempts.len() || attempts[cursor].operation_tag() != 8 {
                return Err(
                    "bounded filesystem replay requires ordered source-input events".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
            while cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
                cursor += 1;
            }
            if cursor == attempts.len()
                || attempts[cursor].operation_tag() != 8
                || !source_directory_chain_is_exact(&attempts[event_start..=cursor])
            {
                return Err(
                    "bounded filesystem replay Source directory chain is inconsistent".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        let reads_start = cursor;
        while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 4 | 6) {
            cursor += 1;
        }
        if cursor == reads_start
            || cursor == attempts.len()
            || attempts[cursor].operation_tag() != 8
        {
            return Err(
                "bounded filesystem replay requires ordered source-input events".to_owned(),
            );
        }
        cursor += 1;
        event_count += 1;
    }
    if event_count == 0 {
        return Err("bounded filesystem replay requires source-input events".to_owned());
    }
    for (index, attempt) in attempts.iter().enumerate() {
        if !matches!(
            attempt.outcome,
            Some(FilesystemOperationAttemptOutcome::Returned { .. })
        ) {
            return Err(format!(
                "filesystem replay event {index} did not return normally"
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_filesystem_replay_size(
    attempts: &[FilesystemOperationAttempt],
) -> Result<(), String> {
    let mut retained = attempts
        .len()
        .checked_mul(FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT);
    let mut add = |weight: usize| {
        retained = retained
            .and_then(|total| total.checked_add(weight))
            .filter(|total| *total <= MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT);
    };
    let lane_weight = |length: usize, weight: usize| length.saturating_mul(weight);
    for attempt in attempts {
        add(lane_weight(
            attempt.scalar_operands.len(),
            FILESYSTEM_REPLAY_SCALAR_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.byte_operands.len(),
            FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.path_like_operands.len(),
            FILESYSTEM_REPLAY_PATH_LIKE_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.rooted_path_operand_resolutions.len(),
            FILESYSTEM_REPLAY_ROOTED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.returned_paths.len(),
            FILESYSTEM_REPLAY_RETURNED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.observed_byte_regions.len(),
            FILESYSTEM_REPLAY_OBSERVED_REGION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.metadata_observations.len(),
            FILESYSTEM_REPLAY_METADATA_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_byte_operand_resolutions.len(),
            FILESYSTEM_REPLAY_MUTABLE_BYTE_RESOLUTION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_i64_operand_resolutions.len(),
            FILESYSTEM_REPLAY_MUTABLE_I64_RESOLUTION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_byte_operands.len(),
            FILESYSTEM_REPLAY_MUTABLE_BYTE_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_i64_operands.len(),
            FILESYSTEM_REPLAY_MUTABLE_I64_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.authorized_paths.len(),
            FILESYSTEM_REPLAY_AUTHORIZED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.logical_handle_inputs.len(),
            FILESYSTEM_REPLAY_LOGICAL_INPUT_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.retired_logical_handles.len(),
            FILESYSTEM_REPLAY_RETIRED_HANDLE_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.grant_refusals.len(),
            FILESYSTEM_REPLAY_GRANT_REFUSAL_RETENTION_WEIGHT,
        ));
        for operand in &attempt.byte_operands {
            add(operand.bytes.len());
        }
        for operand in &attempt.path_like_operands {
            add(operand.bytes.len());
        }
        for operand in &attempt.rooted_path_operand_resolutions {
            add(operand.relative_path.len());
        }
        for returned in &attempt.returned_paths {
            add(returned.bytes.len());
        }
        for operand in &attempt.mutable_byte_operand_resolutions {
            add(operand.bytes.len());
        }
        for operand in &attempt.mutable_byte_operands {
            add(operand.pre_bytes.len());
            add(operand.post_bytes.len());
        }
        for path in &attempt.authorized_paths {
            add(path.relative_path.len());
        }
    }
    retained
        .map(|_| ())
        .ok_or_else(|| format!(
            "filesystem replay attempts exceed their {MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT}-unit deterministic retention-weight ceiling"
        ))
}

pub(crate) fn source_input_record_attempts(
    record: FilesystemSourceInputReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let attempt_count = record.events.iter().fold(0usize, |count, event| {
        count
            + match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => chain.reads.len() + 2,
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => chain
                    .attempt_count()
                    .expect("typed directory replay count fits"),
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => 1,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(_) => 3,
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => 1,
            }
    });
    let mut attempts = Vec::with_capacity(attempt_count);
    for event in record.events {
        match event {
            FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                attempts.extend(source_read_chain_attempts(chain));
            }
            FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                attempts.extend(source_directory_chain_attempts(chain));
            }
            FilesystemSourceInputReplayEventRecord::ReadLink(read_link) => {
                attempts.push(source_read_link_attempt(read_link));
            }
            FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                attempts.extend(source_descriptor_metadata_attempts(metadata));
            }
            FilesystemSourceInputReplayEventRecord::PathMetadata(metadata) => {
                attempts.push(source_path_metadata_attempt(metadata));
            }
        }
    }
    attempts
}

pub(crate) fn source_attempts_overlap_output(
    attempts: &[FilesystemOperationAttempt],
    output_root: FilesystemGrantRootIdentity,
    output_identity: FilesystemLogicalHandleIdentity,
) -> bool {
    attempts.iter().any(|attempt| {
        attempt
            .rooted_path_operand_resolutions
            .iter()
            .any(|path| path.root == output_root)
            || attempt
                .authorized_paths
                .iter()
                .any(|path| path.root == output_root)
            || attempt.logical_handle_output.is_some_and(|output| {
                output.identity == output_identity
                    || matches!(
                        output.source,
                        FilesystemLogicalHandleOutputSource::Duplicated(source)
                            | FilesystemLogicalHandleOutputSource::Borrowed(source)
                            if source == output_identity
                    )
            })
            || attempt.logical_handle_inputs.iter().any(|input| {
                input.resolution
                    == FilesystemLogicalHandleInputResolution::Resolved(output_identity)
            })
            || attempt.retired_logical_handles.contains(&output_identity)
            || attempt.result() == Some(FilesystemOperationResult::LogicalHandle(output_identity))
    })
}

pub(crate) fn output_file_record_from_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<FilesystemOutputFileReplayRecord, String> {
    let Some((create, remainder)) = attempts.split_first() else {
        return Err("bounded filesystem replay requires a complete Output file".to_owned());
    };
    let Some((close, operations)) = remainder.split_last() else {
        return Err("bounded filesystem replay requires a complete Output file".to_owned());
    };

    let [create_mode] = create.scalar_operands.as_slice() else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let [rooted] = create.rooted_path_operand_resolutions.as_slice() else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let [authorized] = create.authorized_paths.as_slice() else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let Some(logical_output) = create.logical_handle_output else {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::LogicalHandle(create_result),
        post_error: create_post_error,
    }) = create.outcome
    else {
        return Err("filesystem replay Output create must succeed".to_owned());
    };
    if create.operation_tag != 1
        || create.provider != FilesystemObservationProvider::RealScoped
        || create_mode.operand_ordinal != 1
        || create_mode.value
            != FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE)
        || rooted.operand_ordinal != 0
        || !filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        || authorized.operand_ordinal != 0
        || authorized.access != FilesystemGrantAccess::Write
        || authorized.root != rooted.root
        || authorized.relative_path != rooted.relative_path
        || logical_output.kind != FilesystemLogicalHandleKind::Descriptor
        || logical_output.identity != create_result
        || logical_output.source != FilesystemLogicalHandleOutputSource::Created
        || !create.byte_operands.is_empty()
        || !create.path_like_operands.is_empty()
        || !create.returned_paths.is_empty()
        || !create.observed_byte_regions.is_empty()
        || !create.metadata_observations.is_empty()
        || !create.mutable_byte_operand_resolutions.is_empty()
        || !create.mutable_i64_operand_resolutions.is_empty()
        || !create.mutable_byte_operands.is_empty()
        || !create.mutable_i64_operands.is_empty()
        || !create.logical_handle_inputs.is_empty()
        || !create.retired_logical_handles.is_empty()
        || !create.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output create lanes are inconsistent".to_owned());
    }

    let mut operation_records = Vec::new();
    operation_records
        .try_reserve_exact(operations.len())
        .map_err(|_| "filesystem replay Output operation allocation failed".to_owned())?;
    let mut operation_cursor = 0;
    while operation_cursor < operations.len() {
        let operation = &operations[operation_cursor];
        if operation.operation_tag == 45 {
            let close_duplicate = operations.get(operation_cursor + 1).ok_or_else(|| {
                "filesystem replay Output duplicate is not immediately retired".to_owned()
            })?;
            operation_records.push(
                FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    output_duplicate_record_from_attempts(
                        operation,
                        close_duplicate,
                        create_result,
                    )?,
                ),
            );
            operation_cursor += 2;
            continue;
        }
        if operation.operation_tag == 46 {
            let release = operations.get(operation_cursor + 1).ok_or_else(|| {
                "filesystem replay Output lock is not immediately released".to_owned()
            })?;
            operation_records.push(FilesystemOutputFileOperationReplayRecord::LockAndUnlock(
                output_lock_record_from_attempts(operation, release, create_result)?,
            ));
            operation_cursor += 2;
            continue;
        }
        operation_records.push(match operation.operation_tag {
            5 | 7 => FilesystemOutputFileOperationReplayRecord::Write(
                output_write_record_from_attempt(operation, create_result)?,
            ),
            10 => output_seek_record_from_attempt(operation, create_result)?,
            17 => output_set_file_permissions_record_from_attempt(operation, create_result)?,
            41 => output_set_length_record_from_attempt(operation, create_result)?,
            42 => output_set_file_times_record_from_attempt(operation, create_result)?,
            43 | 44 => output_sync_record_from_attempt(operation, create_result)?,
            49 => FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                output_change_file_owner_record_from_attempt(operation, create_result)?,
            ),
            _ => return Err("filesystem replay Output operation is unsupported".to_owned()),
        });
        operation_cursor += 1;
    }

    let [close_input] = close.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    };
    let [retired] = close.retired_logical_handles.as_slice() else {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(0),
        post_error: close_post_error,
    }) = close.outcome
    else {
        return Err("filesystem replay Output close must succeed".to_owned());
    };
    if close.operation_tag != 8
        || close.provider != FilesystemObservationProvider::RealScoped
        || close_input.operand_ordinal != 0
        || close_input.kind != FilesystemLogicalHandleKind::Descriptor
        || close_input.resolution != FilesystemLogicalHandleInputResolution::Resolved(create_result)
        || *retired != create_result
        || !close.scalar_operands.is_empty()
        || !close.byte_operands.is_empty()
        || !close.path_like_operands.is_empty()
        || !close.rooted_path_operand_resolutions.is_empty()
        || !close.returned_paths.is_empty()
        || !close.observed_byte_regions.is_empty()
        || !close.metadata_observations.is_empty()
        || !close.mutable_byte_operand_resolutions.is_empty()
        || !close.mutable_i64_operand_resolutions.is_empty()
        || !close.mutable_byte_operands.is_empty()
        || !close.mutable_i64_operands.is_empty()
        || !close.authorized_paths.is_empty()
        || close.logical_handle_output.is_some()
        || !close.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    }

    FilesystemOutputFileReplayRecord::with_operations(
        rooted.root,
        rooted.relative_path.clone(),
        create_result.get(),
        create_post_error,
        operation_records,
        close_post_error,
    )
}

pub(crate) fn output_seek_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I64(offset),
        },
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::I32(whence),
        },
    ] = operation.scalar_operands.as_slice()
    else {
        return Err("filesystem replay Output seek has no exact offset and whence".to_owned());
    };
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output seek lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationResult::Scalar(result)) = operation.result() else {
        return Err("filesystem replay Output seek did not return a scalar".to_owned());
    };
    if !matches!(*whence, 0..=2)
        || result < 0
        || operation.provider != FilesystemObservationProvider::RealScoped
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output seek lanes are inconsistent".to_owned());
    }
    Ok(FilesystemOutputFileOperationReplayRecord::Seek {
        offset: *offset,
        whence: *whence,
        result,
    })
}

pub(crate) fn output_set_length_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I64(length),
        },
    ] = operation.scalar_operands.as_slice()
    else {
        return Err("filesystem replay Output set_len has no exact length".to_owned());
    };
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output set_len lanes are inconsistent".to_owned());
    };
    if *length < 0
        || usize::try_from(*length).is_err()
        || operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output set_len lanes are inconsistent".to_owned());
    }
    Ok(FilesystemOutputFileOperationReplayRecord::SetLength { length: *length })
}

pub(crate) fn output_set_file_permissions_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::U32(mode),
        },
    ] = operation.scalar_operands.as_slice()
    else {
        return Err("filesystem replay Output set_file_permissions has no exact mode".to_owned());
    };
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err(
            "filesystem replay Output set_file_permissions lanes are inconsistent".to_owned(),
        );
    };
    if operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err(
            "filesystem replay Output set_file_permissions lanes are inconsistent".to_owned(),
        );
    }
    Ok(FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode: *mode })
}

pub(crate) fn output_set_file_times_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output set_file_times lanes are inconsistent".to_owned());
    };
    let [resolution] = operation.mutable_byte_operand_resolutions.as_slice() else {
        return Err(
            "filesystem replay Output set_file_times has no exact input carrier".to_owned(),
        );
    };
    let [carrier] = operation.mutable_byte_operands.as_slice() else {
        return Err(
            "filesystem replay Output set_file_times has no exact provider carrier".to_owned(),
        );
    };
    if operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || resolution.operand_ordinal != 1
        || carrier.operand_ordinal != 1
        || resolution.bytes.len() < 32
        || resolution.bytes != carrier.pre_bytes
        || carrier.pre_bytes != carrier.post_bytes
        || !operation.scalar_operands.is_empty()
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output set_file_times lanes are inconsistent".to_owned());
    }
    Ok(FilesystemOutputFileOperationReplayRecord::SetFileTimes {
        times: resolution.bytes.clone(),
    })
}

pub(crate) fn output_sync_record_from_attempt(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputFileOperationReplayRecord, String> {
    let [input] = operation.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output sync lanes are inconsistent".to_owned());
    };
    if !matches!(operation.operation_tag, 43 | 44)
        || operation.provider != FilesystemObservationProvider::RealScoped
        || operation.result() != Some(FilesystemOperationResult::Scalar(0))
        || operation.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !operation.scalar_operands.is_empty()
        || !operation.byte_operands.is_empty()
        || !operation.path_like_operands.is_empty()
        || !operation.rooted_path_operand_resolutions.is_empty()
        || !operation.returned_paths.is_empty()
        || !operation.observed_byte_regions.is_empty()
        || !operation.metadata_observations.is_empty()
        || !operation.mutable_byte_operand_resolutions.is_empty()
        || !operation.mutable_i64_operand_resolutions.is_empty()
        || !operation.mutable_byte_operands.is_empty()
        || !operation.mutable_i64_operands.is_empty()
        || !operation.authorized_paths.is_empty()
        || operation.logical_handle_output.is_some()
        || !operation.retired_logical_handles.is_empty()
        || !operation.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output sync lanes are inconsistent".to_owned());
    }
    Ok(if operation.operation_tag == 43 {
        FilesystemOutputFileOperationReplayRecord::Sync
    } else {
        FilesystemOutputFileOperationReplayRecord::SyncData
    })
}

pub(crate) fn output_write_record_from_attempt(
    write: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputWriteReplayRecord, String> {
    let [write_bytes] = write.byte_operands.as_slice() else {
        return Err("filesystem replay Output write lanes are inconsistent".to_owned());
    };
    let [write_input] = write.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output write lanes are inconsistent".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(write_result),
        post_error: write_post_error,
    }) = write.outcome
    else {
        return Err("filesystem replay Output write must succeed".to_owned());
    };
    let kind_is_exact = match write.operation_tag {
        5 => write.scalar_operands.is_empty(),
        7 => matches!(
            write.scalar_operands.as_slice(),
            [FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I64(offset),
            }] if *offset >= 0
        ),
        _ => false,
    };
    if !kind_is_exact
        || write.provider != FilesystemObservationProvider::RealScoped
        || write_bytes.operand_ordinal != 1
        || write_input.operand_ordinal != 0
        || write_input.kind != FilesystemLogicalHandleKind::Descriptor
        || write_input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || !write.path_like_operands.is_empty()
        || !write.rooted_path_operand_resolutions.is_empty()
        || !write.returned_paths.is_empty()
        || !write.observed_byte_regions.is_empty()
        || !write.metadata_observations.is_empty()
        || !write.mutable_byte_operand_resolutions.is_empty()
        || !write.mutable_i64_operand_resolutions.is_empty()
        || !write.mutable_byte_operands.is_empty()
        || !write.mutable_i64_operands.is_empty()
        || !write.authorized_paths.is_empty()
        || write.logical_handle_output.is_some()
        || !write.retired_logical_handles.is_empty()
        || !write.grant_refusals.is_empty()
    {
        return Err("filesystem replay Output write lanes are inconsistent".to_owned());
    }
    match write.operation_tag {
        5 => FilesystemOutputWriteReplayRecord::new(
            write_bytes.bytes.clone(),
            write_result,
            write_post_error,
        ),
        7 => {
            let [
                FilesystemScalarOperand {
                    value: FilesystemScalarOperandValue::I64(offset),
                    ..
                },
            ] = write.scalar_operands.as_slice()
            else {
                unreachable!("validated positioned write has one i64 offset")
            };
            FilesystemOutputWriteReplayRecord::positioned(
                *offset,
                write_bytes.bytes.clone(),
                write_result,
                write_post_error,
            )
        }
        _ => unreachable!("validated output write has a supported operation"),
    }
}

pub(crate) fn output_file_attempt_end(
    attempts: &[FilesystemOperationAttempt],
    start: usize,
) -> Result<usize, String> {
    if attempts
        .get(start)
        .is_none_or(|attempt| attempt.operation_tag() != 1)
    {
        return Err("filesystem replay Output file must begin with create".to_owned());
    }
    let Some(root_identity) = attempts[start]
        .logical_handle_output
        .map(|output| output.identity)
    else {
        return Err("filesystem replay Output create has no descriptor identity".to_owned());
    };
    let mut cursor = start + 1;
    loop {
        if cursor == attempts.len() {
            return Err(
                "bounded filesystem replay requires complete create-operation*-close Output files"
                    .to_owned(),
            );
        }
        if matches!(
            attempts[cursor].operation_tag(),
            5 | 7 | 10 | 17 | 41 | 42 | 43 | 44 | 49
        ) {
            cursor += 1;
            continue;
        }
        if attempts[cursor].operation_tag() == 45 {
            if cursor + 1 >= attempts.len() || attempts[cursor + 1].operation_tag() != 8 {
                return Err(
                    "filesystem replay Output duplicate must be immediately retired".to_owned(),
                );
            }
            cursor += 2;
            continue;
        }
        if attempts[cursor].operation_tag() == 46 {
            if cursor + 1 >= attempts.len() || attempts[cursor + 1].operation_tag() != 46 {
                return Err("filesystem replay Output lock must be immediately released".to_owned());
            }
            cursor += 2;
            continue;
        }
        let closes_root = attempts[cursor].operation_tag() == 8
            && matches!(
                attempts[cursor].logical_handle_inputs.as_slice(),
                [FilesystemLogicalHandleInput {
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                    ..
                }] if *identity == root_identity
            );
        if closes_root {
            return Ok(cursor + 1);
        }
        return Err(
            "bounded filesystem replay requires complete create-operation*-close Output files"
                .to_owned(),
        );
    }
}

pub(crate) fn filesystem_output_attempt_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 1 | 9 | 11 | 12 | 19 | 20 | 27)
}

pub(crate) fn output_absent_remove_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    output_absent_remove_record_from_attempt(attempt).is_ok()
}

pub(crate) fn validate_output_absent_remove_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if attempts.is_empty() {
        return Err("filesystem replay requires at least one absent Output remove".to_owned());
    }
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failure-only Output operations cannot hand off generated sources"
                .to_owned(),
        );
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(attempts.len())
        .map_err(|_| "filesystem replay absent Output remove allocation failed".to_owned())?;
    for attempt in attempts {
        records.push(output_absent_remove_record_from_attempt(attempt)?);
    }
    let record = FilesystemInputOutputAbsentRemovesReplayRecord::new(None, records)?;
    let (_, records) = record.into_parts();
    let output_root = records[0].output_root();
    if source_attempts_use_root(source_attempts, output_root) {
        return Err("filesystem replay Source and Output roots must be distinct".to_owned());
    }
    Ok(())
}

pub(crate) fn output_tree_entries_from_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<Vec<FilesystemOutputTreeEntryReplayRecord>, String> {
    if attempts.is_empty() {
        return Err("bounded filesystem replay requires Output entries".to_owned());
    }
    let mut entries = Vec::new();
    let mut cursor = 0;
    while cursor < attempts.len() {
        match attempts[cursor].operation_tag() {
            11 => {
                entries.push(FilesystemOutputTreeEntryReplayRecord::Directory(
                    output_directory_record_from_attempt(&attempts[cursor])?,
                ));
                cursor += 1;
            }
            1 => {
                let end = output_file_attempt_end(attempts, cursor)?;
                entries.push(FilesystemOutputTreeEntryReplayRecord::File(
                    output_file_record_from_attempts(&attempts[cursor..end])?,
                ));
                cursor = end;
            }
            20 => {
                entries.push(FilesystemOutputTreeEntryReplayRecord::Symlink(
                    output_symlink_record_from_attempt(&attempts[cursor])?,
                ));
                cursor += 1;
            }
            19 | 27 => {
                entries.push(FilesystemOutputTreeEntryReplayRecord::HardLink(
                    output_hard_link_record_from_attempt(&attempts[cursor])?,
                ));
                cursor += 1;
            }
            _ => {
                return Err("bounded filesystem replay requires ordered Output entries".to_owned());
            }
        }
    }
    Ok(entries)
}

pub(crate) fn output_file_attempts(
    record: FilesystemOutputFileReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let create = FilesystemOperationAttempt {
        operation_tag: 1,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error: record.create_post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.output_root,
            relative_path: record.output_relative_path.clone(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Write,
            root: record.output_root,
            relative_path: record.output_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    };
    let operation_capacity = record
        .operations
        .iter()
        .map(output_file_operation_attempt_count)
        .sum();
    let mut operations = Vec::with_capacity(operation_capacity);
    for operation in record.operations {
        match operation {
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate) => {
                operations.extend(output_duplicate_attempts(identity, duplicate));
            }
            FilesystemOutputFileOperationReplayRecord::LockAndUnlock(lock) => {
                operations.extend(output_lock_attempts(identity, lock));
            }
            operation => operations.push(output_file_operation_attempt(operation, identity)),
        }
    }
    let close = FilesystemOperationAttempt {
        operation_tag: 8,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.close_post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![identity],
        grant_refusals: Vec::new(),
    };
    let mut attempts = Vec::with_capacity(operations.len() + 2);
    attempts.push(create);
    attempts.extend(operations);
    attempts.push(close);
    attempts
}

pub(crate) fn output_file_operation_attempt(
    operation: FilesystemOutputFileOperationReplayRecord,
    identity: FilesystemLogicalHandleIdentity,
) -> FilesystemOperationAttempt {
    match operation {
        FilesystemOutputFileOperationReplayRecord::Write(write) => FilesystemOperationAttempt {
            operation_tag: match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => 5,
                FilesystemOutputWriteReplayKind::Positioned { .. } => 7,
            },
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(write.result),
                post_error: write.post_error,
            }),
            scalar_operands: match write.kind {
                FilesystemOutputWriteReplayKind::Sequential => Vec::new(),
                FilesystemOutputWriteReplayKind::Positioned { offset } => {
                    vec![FilesystemScalarOperand {
                        operand_ordinal: 2,
                        value: FilesystemScalarOperandValue::I64(offset),
                    }]
                }
            },
            byte_operands: vec![FilesystemByteOperand {
                operand_ordinal: 1,
                bytes: write.bytes,
            }],
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        },
        FilesystemOutputFileOperationReplayRecord::Seek {
            offset,
            whence,
            result,
        } => FilesystemOperationAttempt {
            operation_tag: 10,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(result),
                post_error: 0,
            }),
            scalar_operands: vec![
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I32(whence),
                },
            ],
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        },
        FilesystemOutputFileOperationReplayRecord::SetLength { length } => {
            FilesystemOperationAttempt {
                operation_tag: 41,
                provider: FilesystemObservationProvider::RealScoped,
                outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: 0,
                }),
                scalar_operands: vec![FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I64(length),
                }],
                byte_operands: Vec::new(),
                path_like_operands: Vec::new(),
                rooted_path_operand_resolutions: Vec::new(),
                returned_paths: Vec::new(),
                observed_byte_regions: Vec::new(),
                metadata_observations: Vec::new(),
                mutable_byte_operand_resolutions: Vec::new(),
                mutable_i64_operand_resolutions: Vec::new(),
                mutable_byte_operands: Vec::new(),
                mutable_i64_operands: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                }],
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }
        FilesystemOutputFileOperationReplayRecord::SetFilePermissions { mode } => {
            FilesystemOperationAttempt {
                operation_tag: 17,
                provider: FilesystemObservationProvider::RealScoped,
                outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: 0,
                }),
                scalar_operands: vec![FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::U32(mode),
                }],
                byte_operands: Vec::new(),
                path_like_operands: Vec::new(),
                rooted_path_operand_resolutions: Vec::new(),
                returned_paths: Vec::new(),
                observed_byte_regions: Vec::new(),
                metadata_observations: Vec::new(),
                mutable_byte_operand_resolutions: Vec::new(),
                mutable_i64_operand_resolutions: Vec::new(),
                mutable_byte_operands: Vec::new(),
                mutable_i64_operands: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                }],
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }
        FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } => {
            let resolution_times = times.clone();
            let pre_times = times.clone();
            FilesystemOperationAttempt {
                operation_tag: 42,
                provider: FilesystemObservationProvider::RealScoped,
                outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: 0,
                }),
                scalar_operands: Vec::new(),
                byte_operands: Vec::new(),
                path_like_operands: Vec::new(),
                rooted_path_operand_resolutions: Vec::new(),
                returned_paths: Vec::new(),
                observed_byte_regions: Vec::new(),
                metadata_observations: Vec::new(),
                mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
                    operand_ordinal: 1,
                    bytes: resolution_times,
                }],
                mutable_i64_operand_resolutions: Vec::new(),
                mutable_byte_operands: vec![FilesystemMutableByteOperand {
                    operand_ordinal: 1,
                    pre_bytes: pre_times,
                    post_bytes: times,
                }],
                mutable_i64_operands: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
                }],
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }
        FilesystemOutputFileOperationReplayRecord::Sync
        | FilesystemOutputFileOperationReplayRecord::SyncData => FilesystemOperationAttempt {
            operation_tag: match operation {
                FilesystemOutputFileOperationReplayRecord::Sync => 43,
                FilesystemOutputFileOperationReplayRecord::SyncData => 44,
                FilesystemOutputFileOperationReplayRecord::Write(_)
                | FilesystemOutputFileOperationReplayRecord::Seek { .. }
                | FilesystemOutputFileOperationReplayRecord::SetLength { .. }
                | FilesystemOutputFileOperationReplayRecord::SetFilePermissions { .. }
                | FilesystemOutputFileOperationReplayRecord::SetFileTimes { .. }
                | FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(_)
                | FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_)
                | FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_) => {
                    unreachable!()
                }
            },
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(0),
                post_error: 0,
            }),
            scalar_operands: Vec::new(),
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: Vec::new(),
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: Vec::new(),
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: Vec::new(),
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        },
        FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(ownership) => {
            output_change_file_owner_attempt(identity, ownership)
        }
        FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_) => {
            unreachable!("duplicate pairs are expanded by output_file_attempts")
        }
        FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_) => {
            unreachable!("lock pairs are expanded by output_file_attempts")
        }
    }
}

#[cfg(test)]
mod record_tests;

pub(crate) fn source_path_metadata_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    let expected_kind = match attempt.operation_tag {
        38 => FilesystemMetadataObservationKind::FollowedPath,
        40 => FilesystemMetadataObservationKind::UnfollowedFinalPath,
        _ => return false,
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return false;
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return false;
    };
    let [metadata] = attempt.metadata_observations.as_slice() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands.as_slice() else {
        return false;
    };
    attempt.provider == FilesystemObservationProvider::RealScoped
        && attempt.result() == Some(FilesystemOperationResult::Scalar(0))
        && attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && rooted.operand_ordinal == 0
        && filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        && attempt.returned_paths.is_empty()
        && attempt.observed_byte_regions.is_empty()
        && metadata.output_operand_ordinal == 1
        && metadata.kind == expected_kind
        && mutable_resolution.operand_ordinal == 1
        && mutable.operand_ordinal == 1
        && mutable_resolution.bytes == mutable.pre_bytes
        && mutable.pre_bytes.len() == mutable.post_bytes.len()
        && mutable.post_bytes.len() >= FILESYSTEM_METADATA_API_CARRIER_BYTES
        && authorized.operand_ordinal == 0
        && authorized.access == FilesystemGrantAccess::Read
        && authorized.root == rooted.root
        && filesystem_root_relative_path_is_canonical(&authorized.relative_path, true)
        && attempt.mutable_i64_operand_resolutions.is_empty()
        && attempt.mutable_i64_operands.is_empty()
        && attempt.logical_handle_inputs.is_empty()
        && attempt.logical_handle_output.is_none()
        && attempt.retired_logical_handles.is_empty()
        && attempt.grant_refusals.is_empty()
}

pub(crate) fn source_path_metadata_attempt(
    record: FilesystemSourcePathMetadataReplayRecord,
) -> FilesystemOperationAttempt {
    let operation_tag = match record.kind {
        FilesystemMetadataObservationKind::FollowedPath => 38,
        FilesystemMetadataObservationKind::UnfollowedFinalPath => 40,
        FilesystemMetadataObservationKind::OpenDescriptor => {
            unreachable!("validated source path metadata cannot target a descriptor")
        }
    };
    FilesystemOperationAttempt {
        operation_tag,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.source_root,
            relative_path: record.source_relative_path,
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: vec![record.metadata],
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.mutable_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.mutable_pre_state,
            post_bytes: record.mutable_post_state,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Read,
            root: record.authorized_root,
            relative_path: record.authorized_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

pub(crate) fn source_read_chain_attempts(
    record: FilesystemSourceReadChainReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let open = source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let read_count = record.reads.len();
    let reads = record.reads.into_iter().map(|read| {
        let read_length =
            usize::try_from(read.read_result).expect("validated replay read result fits usize");
        let (operation_tag, scalar_operands, region_kind) = match read.read_kind {
            FilesystemReplayReadKind::Positioned { offset } => (
                6,
                vec![
                    FilesystemScalarOperand {
                        operand_ordinal: 2,
                        value: FilesystemScalarOperandValue::U64(read.requested_count),
                    },
                    FilesystemScalarOperand {
                        operand_ordinal: 3,
                        value: FilesystemScalarOperandValue::I64(offset),
                    },
                ],
                FilesystemObservedByteRegionKind::PositionedFileRead,
            ),
            FilesystemReplayReadKind::Sequential => (
                4,
                vec![FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(read.requested_count),
                }],
                FilesystemObservedByteRegionKind::SequentialFileRead,
            ),
        };
        FilesystemOperationAttempt {
            operation_tag,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(read.read_result),
                post_error: read.read_post_error,
            }),
            scalar_operands,
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: vec![FilesystemObservedByteRegion {
                output_operand_ordinal: 1,
                kind: region_kind,
                offset: 0,
                length: read_length,
            }],
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
                operand_ordinal: 1,
                bytes: read.mutable_resolution,
            }],
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: vec![FilesystemMutableByteOperand {
                operand_ordinal: 1,
                pre_bytes: read.mutable_pre_state,
                post_bytes: read.mutable_post_state,
            }],
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }
    });
    let close = source_descriptor_close_attempt(identity, record.close_post_error);
    let mut attempts = Vec::with_capacity(read_count + 2);
    attempts.push(open);
    attempts.extend(reads);
    attempts.push(close);
    attempts
}

pub(crate) fn source_descriptor_metadata_attempts(
    record: FilesystemSourceDescriptorMetadataReplayRecord,
) -> [FilesystemOperationAttempt; 3] {
    let identity = record.logical_handle_identity;
    let open = source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let metadata = FilesystemOperationAttempt {
        operation_tag: 39,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.metadata_post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: vec![record.metadata],
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.mutable_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.mutable_pre_state,
            post_bytes: record.mutable_post_state,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    };
    let close = source_descriptor_close_attempt(identity, record.close_post_error);
    [open, metadata, close]
}

pub(crate) fn source_descriptor_open_attempt(
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: 2,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: source_root,
            relative_path: source_relative_path.clone(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Read,
            root: source_root,
            relative_path: source_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

pub(crate) fn source_descriptor_close_attempt(
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: 8,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![identity],
        grant_refusals: Vec::new(),
    }
}
