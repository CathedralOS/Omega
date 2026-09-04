use crate::{
    FilesystemAuthorizedPath, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemReturnedPath, FilesystemReturnedPathCompleteness,
    FilesystemReturnedPathKind, FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
    FilesystemScalarOperandValue, filesystem_root_relative_path_is_canonical,
};

const READ_LINK_OPERATION_TAG: u16 = 21;

/// One exact successful Source-rooted `read_link` event.
///
/// A limited result retains only the authoritative prefix returned by the
/// provider. No absent target suffix is inferred or admitted into replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceReadLinkReplayRecord {
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    authorized_root: FilesystemGrantRootIdentity,
    authorized_relative_path: Vec<u8>,
    requested_count: u64,
    result: i64,
    post_error: i32,
    mutable_resolution: Vec<u8>,
    mutable_pre_state: Vec<u8>,
    mutable_post_state: Vec<u8>,
    completeness: FilesystemReturnedPathCompleteness,
    returned_bytes: Vec<u8>,
}

impl FilesystemSourceReadLinkReplayRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        authorized_root: FilesystemGrantRootIdentity,
        authorized_relative_path: Vec<u8>,
        requested_count: u64,
        result: i64,
        post_error: i32,
        mutable_resolution: Vec<u8>,
        mutable_pre_state: Vec<u8>,
        mutable_post_state: Vec<u8>,
        completeness: FilesystemReturnedPathCompleteness,
        returned_bytes: Vec<u8>,
    ) -> Result<Self, String> {
        if source_root != authorized_root
            || !filesystem_root_relative_path_is_canonical(&source_relative_path, false)
            || !filesystem_root_relative_path_is_canonical(&authorized_relative_path, true)
            || !read_link_payload_is_consistent(
                requested_count,
                result,
                &mutable_resolution,
                &mutable_pre_state,
                &mutable_post_state,
                completeness,
                &returned_bytes,
            )
        {
            return Err("filesystem replay Source read-link event is inconsistent".to_owned());
        }
        Ok(Self {
            source_root,
            source_relative_path,
            authorized_root,
            authorized_relative_path,
            requested_count,
            result,
            post_error,
            mutable_resolution,
            mutable_pre_state,
            mutable_post_state,
            completeness,
            returned_bytes,
        })
    }

    pub const fn source_root(&self) -> FilesystemGrantRootIdentity {
        self.source_root
    }

    pub fn source_relative_path(&self) -> &[u8] {
        &self.source_relative_path
    }

    pub const fn authorized_root(&self) -> FilesystemGrantRootIdentity {
        self.authorized_root
    }

    pub fn authorized_relative_path(&self) -> &[u8] {
        &self.authorized_relative_path
    }

    pub const fn requested_count(&self) -> u64 {
        self.requested_count
    }

    pub const fn result(&self) -> i64 {
        self.result
    }

    pub const fn post_error(&self) -> i32 {
        self.post_error
    }

    pub fn mutable_resolution(&self) -> &[u8] {
        &self.mutable_resolution
    }

    pub fn mutable_pre_state(&self) -> &[u8] {
        &self.mutable_pre_state
    }

    pub fn mutable_post_state(&self) -> &[u8] {
        &self.mutable_post_state
    }

    pub const fn completeness(&self) -> FilesystemReturnedPathCompleteness {
        self.completeness
    }

    pub fn returned_bytes(&self) -> &[u8] {
        &self.returned_bytes
    }
}

pub(crate) fn source_read_link_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(result),
        ..
    }) = attempt.outcome
    else {
        return false;
    };
    let [
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(requested_count),
        },
    ] = attempt.scalar_operands.as_slice()
    else {
        return false;
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return false;
    };
    let [returned] = attempt.returned_paths.as_slice() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands.as_slice() else {
        return false;
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return false;
    };

    attempt.operation_tag == READ_LINK_OPERATION_TAG
        && attempt.provider == FilesystemObservationProvider::RealScoped
        && rooted.operand_ordinal == 0
        && filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        && authorized.operand_ordinal == 0
        && authorized.access == FilesystemGrantAccess::Read
        && authorized.root == rooted.root
        && filesystem_root_relative_path_is_canonical(&authorized.relative_path, true)
        && returned.operand_ordinal == 1
        && returned.kind == FilesystemReturnedPathKind::ReadLinkPayload
        && mutable_resolution.operand_ordinal == 1
        && mutable.operand_ordinal == 1
        && read_link_payload_is_consistent(
            *requested_count,
            result,
            &mutable_resolution.bytes,
            &mutable.pre_bytes,
            &mutable.post_bytes,
            returned.completeness,
            &returned.bytes,
        )
        && only_read_link_lanes(attempt)
}

pub(crate) fn source_read_link_attempt(
    record: FilesystemSourceReadLinkReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: READ_LINK_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(record.result),
            post_error: record.post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(record.requested_count),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.source_root,
            relative_path: record.source_relative_path,
        }],
        returned_paths: vec![FilesystemReturnedPath {
            operand_ordinal: 1,
            kind: FilesystemReturnedPathKind::ReadLinkPayload,
            completeness: record.completeness,
            bytes: record.returned_bytes,
        }],
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
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

fn read_link_payload_is_consistent(
    requested_count: u64,
    result: i64,
    mutable_resolution: &[u8],
    mutable_pre_state: &[u8],
    mutable_post_state: &[u8],
    completeness: FilesystemReturnedPathCompleteness,
    returned_bytes: &[u8],
) -> bool {
    let Ok(result_length) = usize::try_from(result) else {
        return false;
    };
    let Ok(requested_capacity) = usize::try_from(requested_count) else {
        return false;
    };
    mutable_resolution.len() == mutable_pre_state.len()
        && mutable_pre_state.len() == mutable_post_state.len()
        && requested_capacity <= mutable_pre_state.len()
        && result_length == returned_bytes.len()
        && result_length <= requested_capacity
        && mutable_post_state[..result_length] == *returned_bytes
        && mutable_post_state[result_length..] == mutable_pre_state[result_length..]
        && (completeness == FilesystemReturnedPathCompleteness::Complete
            || result_length == requested_capacity)
}

fn only_read_link_lanes(attempt: &FilesystemOperationAttempt) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.observed_byte_regions.is_empty()
        && attempt.metadata_observations.is_empty()
        && attempt.mutable_i64_operand_resolutions.is_empty()
        && attempt.mutable_i64_operands.is_empty()
        && attempt.logical_handle_inputs.is_empty()
        && attempt.logical_handle_output.is_none()
        && attempt.retired_logical_handles.is_empty()
        && attempt.grant_refusals.is_empty()
}
