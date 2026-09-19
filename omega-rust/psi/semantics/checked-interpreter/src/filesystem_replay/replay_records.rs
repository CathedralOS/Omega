//! The typed source and output replay records and their attempt counts.

use crate::filesystem_replay::duplicates::{
    FilesystemOutputDuplicateReplayRecord, MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES,
    output_logical_handle_identities, validate_output_duplicate_replay,
};
use crate::filesystem_replay::locks::{
    FilesystemOutputLockReplayRecord, validate_output_lock_replay,
};
use crate::filesystem_replay::native_query_chains::FilesystemSourceNativeHandleQueryChainReplayRecord;
use crate::filesystem_replay::output_ownership::FilesystemOutputChangeFileOwnerReplayRecord;
use crate::filesystem_replay::replay_validation::validate_expected_included_sources;
use crate::filesystem_replay::source_directories::FilesystemSourceDirectoryReadChainReplayRecord;
use crate::filesystem_replay::source_read_links::FilesystemSourceReadLinkReplayRecord;
use crate::filesystem_replay::{
    FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE, MAX_FILESYSTEM_REPLAY_RETAINED_BYTES,
};
use crate::{
    BuildIncludedSource, FILESYSTEM_METADATA_API_CARRIER_BYTES, FilesystemGrantRootIdentity,
    FilesystemLogicalHandleIdentity, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, filesystem_root_relative_path_is_canonical,
};

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
    /// One bounded `open_path_handle` / query / `close_handle` lifecycle on
    /// one Source object under the constrained query-only contract.
    NativeHandleQueryChain(FilesystemSourceNativeHandleQueryChainReplayRecord),
}

/// Typed source-input replay record reconstructed after canonical bytes cross
/// a process boundary. It grants no ambient host filesystem authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceInputReplayRecord {
    pub(crate) events: Vec<FilesystemSourceInputReplayEventRecord>,
}

impl FilesystemSourceInputReplayRecord {
    /// Expand exact typed events without cloning their retained input carriers.
    /// Event order here is construction order; a mixed stream retains its own
    /// original attempt ordinals and places these operations at those positions.
    pub fn into_attempts(self) -> Vec<crate::FilesystemOperationAttempt> {
        super::source_attempts::source_input_record_attempts(self)
    }

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
                FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain) => {
                    Some(chain.logical_handle_identity)
                }
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
                    FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain) => {
                        chain.attempt_count()
                    }
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
                FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain) => {
                    Some(chain.logical_handle_identity)
                }
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
                FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain) => {
                    chain.source_root == output.output_root
                        || chain.logical_handle_identity == output.logical_handle_identity
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
