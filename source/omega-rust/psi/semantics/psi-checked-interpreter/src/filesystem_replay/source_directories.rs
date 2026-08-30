use crate::{
    FilesystemGrantAccess, FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemLogicalHandleOutputSource, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemMutableI64Operand,
    FilesystemMutableI64OperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemObservedByteRegionKind, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemScalarOperand,
    FilesystemScalarOperandValue, filesystem_root_relative_path_is_canonical,
};

const READ_DIRECTORY_OPERATION_TAG: u16 = 23;

/// One exact successful call within a Source directory-enumeration chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceDirectoryReadReplayRecord {
    requested_count: u64,
    result: i64,
    post_error: i32,
    mutable_resolution: Vec<u8>,
    mutable_pre_state: Vec<u8>,
    mutable_post_state: Vec<u8>,
    position_resolution: i64,
    position_pre_state: i64,
    position_post_state: i64,
}

impl FilesystemSourceDirectoryReadReplayRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        requested_count: u64,
        result: i64,
        post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
        position_resolution: i64,
        position_pre_state: i64,
        position_post_state: i64,
    ) -> Result<Self, String> {
        let result_length = usize::try_from(result)
            .map_err(|_| "filesystem replay directory result must be nonnegative".to_owned())?;
        let requested_capacity = usize::try_from(requested_count)
            .map_err(|_| "filesystem replay directory request exceeds this host".to_owned())?;
        if mutable_resolution.len() != mutable_pre_state.len()
            || mutable_pre_state.len() != mutable_post_state.len()
            || requested_capacity > mutable_post_state.len()
            || result_length > requested_capacity
            || mutable_pre_state[result_length..] != mutable_post_state[result_length..]
        {
            return Err("filesystem replay Source directory carrier is inconsistent".to_owned());
        }
        Ok(Self {
            requested_count,
            result,
            post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
            position_resolution,
            position_pre_state,
            position_post_state,
        })
    }
}

/// One closed Source directory enumeration: an exact flags-zero open, one or
/// more exact record reads, and the exact successful descriptor retirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceDirectoryReadChainReplayRecord {
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    logical_handle_identity: FilesystemLogicalHandleIdentity,
    open_post_error: i32,
    reads: Vec<FilesystemSourceDirectoryReadReplayRecord>,
    close_post_error: i32,
}

impl FilesystemSourceDirectoryReadChainReplayRecord {
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        open_post_error: i32,
        reads: Vec<FilesystemSourceDirectoryReadReplayRecord>,
        close_post_error: i32,
    ) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay logical identity must be nonzero".to_owned())?;
        if !filesystem_root_relative_path_is_canonical(&source_relative_path, false)
            || reads.is_empty()
            || reads.len().checked_add(2).is_none()
        {
            return Err(
                "filesystem replay requires one canonical Source directory read chain".to_owned(),
            );
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

    pub const fn source_root(&self) -> FilesystemGrantRootIdentity {
        self.source_root
    }

    pub const fn logical_handle_identity(&self) -> FilesystemLogicalHandleIdentity {
        self.logical_handle_identity
    }

    pub fn attempt_count(&self) -> Option<usize> {
        self.reads.len().checked_add(2)
    }
}

pub(crate) fn source_directory_chain_attempts(
    record: FilesystemSourceDirectoryReadChainReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let open = crate::source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let read_count = record.reads.len();
    let reads = record.reads.into_iter().map(|read| {
        let result_length =
            usize::try_from(read.result).expect("validated directory replay result fits usize");
        FilesystemOperationAttempt {
            operation_tag: READ_DIRECTORY_OPERATION_TAG,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(read.result),
                post_error: read.post_error,
            }),
            scalar_operands: vec![FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::U64(read.requested_count),
            }],
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: vec![FilesystemObservedByteRegion {
                output_operand_ordinal: 1,
                kind: FilesystemObservedByteRegionKind::DirectoryRecords,
                offset: 0,
                length: result_length,
            }],
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
                operand_ordinal: 1,
                bytes: read.mutable_resolution,
            }],
            mutable_i64_operand_resolutions: vec![FilesystemMutableI64OperandResolution {
                operand_ordinal: 3,
                value: read.position_resolution,
            }],
            mutable_byte_operands: vec![FilesystemMutableByteOperand {
                operand_ordinal: 1,
                pre_bytes: read.mutable_pre_state,
                post_bytes: read.mutable_post_state,
            }],
            mutable_i64_operands: vec![FilesystemMutableI64Operand {
                operand_ordinal: 3,
                pre_value: read.position_pre_state,
                post_value: read.position_post_state,
            }],
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
    let close = crate::source_descriptor_close_attempt(identity, record.close_post_error);
    let mut attempts = Vec::with_capacity(read_count + 2);
    attempts.push(open);
    attempts.extend(reads);
    attempts.push(close);
    attempts
}

pub(crate) fn source_directory_chain_is_exact(attempts: &[FilesystemOperationAttempt]) -> bool {
    if attempts.len() < 3 {
        return false;
    }
    let open = &attempts[0];
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::LogicalHandle(identity),
        ..
    }) = open.outcome
    else {
        return false;
    };
    if open.operation_tag != 2 {
        return false;
    }
    for read in &attempts[1..attempts.len() - 1] {
        if !source_directory_read_is_exact(read, identity) {
            return false;
        }
    }
    source_directory_open_identity(open) == Some(identity)
        && source_directory_close_is_exact(&attempts[attempts.len() - 1], identity)
}

fn source_directory_open_identity(
    open: &FilesystemOperationAttempt,
) -> Option<FilesystemLogicalHandleIdentity> {
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::LogicalHandle(identity),
        ..
    }) = open.outcome
    else {
        return None;
    };
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        },
    ] = open.scalar_operands.as_slice()
    else {
        return None;
    };
    let [rooted] = open.rooted_path_operand_resolutions.as_slice() else {
        return None;
    };
    let [authorized] = open.authorized_paths.as_slice() else {
        return None;
    };
    let Some(output) = open.logical_handle_output else {
        return None;
    };
    (open.operation_tag == 2
        && open.provider == FilesystemObservationProvider::RealScoped
        && rooted.operand_ordinal == 0
        && filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        && authorized.operand_ordinal == 0
        && authorized.access == FilesystemGrantAccess::Read
        && authorized.root == rooted.root
        && authorized.relative_path == rooted.relative_path
        && output.kind == FilesystemLogicalHandleKind::Descriptor
        && output.identity == identity
        && output.source == FilesystemLogicalHandleOutputSource::Created
        && open.byte_operands.is_empty()
        && open.path_like_operands.is_empty()
        && open.returned_paths.is_empty()
        && open.observed_byte_regions.is_empty()
        && open.metadata_observations.is_empty()
        && open.mutable_byte_operand_resolutions.is_empty()
        && open.mutable_i64_operand_resolutions.is_empty()
        && open.mutable_byte_operands.is_empty()
        && open.mutable_i64_operands.is_empty()
        && open.logical_handle_inputs.is_empty()
        && open.retired_logical_handles.is_empty()
        && open.grant_refusals.is_empty())
    .then_some(identity)
}

fn source_directory_close_is_exact(
    close: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> bool {
    let [handle] = close.logical_handle_inputs.as_slice() else {
        return false;
    };
    close.operation_tag == 8
        && close.provider == FilesystemObservationProvider::RealScoped
        && matches!(
            close.outcome,
            Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(0),
                ..
            })
        )
        && handle.operand_ordinal == 0
        && handle.kind == FilesystemLogicalHandleKind::Descriptor
        && handle.resolution == FilesystemLogicalHandleInputResolution::Resolved(identity)
        && close.retired_logical_handles.as_slice() == [identity]
        && close.scalar_operands.is_empty()
        && close.byte_operands.is_empty()
        && close.path_like_operands.is_empty()
        && close.rooted_path_operand_resolutions.is_empty()
        && close.returned_paths.is_empty()
        && close.observed_byte_regions.is_empty()
        && close.metadata_observations.is_empty()
        && close.mutable_byte_operand_resolutions.is_empty()
        && close.mutable_i64_operand_resolutions.is_empty()
        && close.mutable_byte_operands.is_empty()
        && close.mutable_i64_operands.is_empty()
        && close.authorized_paths.is_empty()
        && close.logical_handle_output.is_none()
        && close.grant_refusals.is_empty()
}

fn source_directory_read_is_exact(
    read: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> bool {
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(result),
        ..
    }) = read.outcome
    else {
        return false;
    };
    let [
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(requested_count),
        },
    ] = read.scalar_operands.as_slice()
    else {
        return false;
    };
    let [region] = read.observed_byte_regions.as_slice() else {
        return false;
    };
    let [resolution] = read.mutable_byte_operand_resolutions.as_slice() else {
        return false;
    };
    let [mutable] = read.mutable_byte_operands.as_slice() else {
        return false;
    };
    let [position_resolution] = read.mutable_i64_operand_resolutions.as_slice() else {
        return false;
    };
    let [position] = read.mutable_i64_operands.as_slice() else {
        return false;
    };
    let [handle] = read.logical_handle_inputs.as_slice() else {
        return false;
    };
    let Ok(record) = FilesystemSourceDirectoryReadReplayRecord::new(
        *requested_count,
        result,
        0,
        resolution.bytes.clone(),
        mutable.pre_bytes.clone(),
        mutable.post_bytes.clone(),
        position_resolution.value,
        position.pre_value,
        position.post_value,
    ) else {
        return false;
    };
    read.operation_tag == READ_DIRECTORY_OPERATION_TAG
        && read.provider == FilesystemObservationProvider::RealScoped
        && region.output_operand_ordinal == 1
        && region.kind == FilesystemObservedByteRegionKind::DirectoryRecords
        && region.offset == 0
        && region.length == usize::try_from(result).unwrap_or(usize::MAX)
        && resolution.operand_ordinal == 1
        && mutable.operand_ordinal == 1
        && position_resolution.operand_ordinal == 3
        && position.operand_ordinal == 3
        && handle.operand_ordinal == 0
        && handle.kind == FilesystemLogicalHandleKind::Descriptor
        && handle.resolution == FilesystemLogicalHandleInputResolution::Resolved(identity)
        && only_directory_read_lanes(read)
        && record.result == result
}

fn only_directory_read_lanes(read: &FilesystemOperationAttempt) -> bool {
    read.byte_operands.is_empty()
        && read.path_like_operands.is_empty()
        && read.rooted_path_operand_resolutions.is_empty()
        && read.returned_paths.is_empty()
        && read.metadata_observations.is_empty()
        && read.authorized_paths.is_empty()
        && read.logical_handle_output.is_none()
        && read.retired_logical_handles.is_empty()
        && read.grant_refusals.is_empty()
}
