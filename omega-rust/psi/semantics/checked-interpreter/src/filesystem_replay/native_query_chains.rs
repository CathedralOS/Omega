//! Bounded occurrence-specific Source native-handle query-release replay
//! records.
//!
//! A query-release chain is one Source-rooted `open_path_handle` acquisition
//! under the constrained query-only contract (`desired_access == 0`, full
//! read/write/delete sharing, null security attributes, `OPEN_EXISTING`,
//! `FILE_FLAG_BACKUP_SEMANTICS` and no `FILE_FLAG_DELETE_ON_CLOSE`), followed
//! by one or more bounded, handle-preserving observations on the exact
//! logical identity, followed by one successful `close_handle` that retires
//! that identity. The admitted intervening operations are
//! `final_path_name_by_handle` observations, which take the handle and
//! return the object's final path, and handle-free `get_last_error` reads,
//! which observe the thread error slot without touching the handle.
//!
//! Every lane is occurrence-specific evidence: the acquisition's exact
//! scalar contract, rooted-path resolution, authorization, and null
//! template handle; each observation's exact buffer custody and returned
//! bytes; the close's nonzero result; and the logical-handle identity
//! carried through each `Resolved` input and `retired` output. No lane may
//! carry a second handle output, an earlier retirement of the identity, an
//! aliased or substituted input, or a deferred-deletion acquisition flag,
//! so the record is the bounded proof that the acquired object was observed
//! and then released exactly once with no later use.

use crate::{
    FilesystemAuthorizedPath, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemReturnedPath, FilesystemReturnedPathCompleteness,
    FilesystemReturnedPathKind, FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
    FilesystemScalarOperandValue, filesystem_root_relative_path_is_canonical,
};

const OPEN_PATH_HANDLE_OPERATION_TAG: u16 = 28;
const CLOSE_HANDLE_OPERATION_TAG: u16 = 29;
const FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG: u16 = 31;
const GET_LAST_ERROR_OPERATION_TAG: u16 = 35;

/// Constrained `open_path_handle` scalar contract for the bounded
/// query-release occurrence. `desired_access == 0` opens a query-only
/// object, share mode `0x7` preserves read/write/delete sharing for other
/// opens, `OPEN_EXISTING` refuses to create a new object, and
/// `FILE_FLAG_BACKUP_SEMANTICS` alone permits directory opens without
/// attaching deferred deletion (`FILE_FLAG_DELETE_ON_CLOSE` stays absent).
const QUERY_OPEN_DESIRED_ACCESS: u32 = 0;
const QUERY_OPEN_SHARE_MODE: u32 = 0x7;
const QUERY_OPEN_SECURITY_ATTRIBUTES: i64 = 0;
const QUERY_OPEN_CREATION_DISPOSITION: u32 = 3;
const QUERY_OPEN_FLAGS_AND_ATTRIBUTES: u32 = 0x0200_0000;

/// One complete `final_path_name_by_handle` observation inside a
/// constrained Source native-handle query-release chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemNativeHandleFinalPathQueryReplayRecord {
    capacity: u64,
    flags: u32,
    result: i64,
    post_error: i32,
    buffer_resolution: Vec<u8>,
    buffer_pre_bytes: Vec<u8>,
    buffer_post_bytes: Vec<u8>,
    returned_path: Vec<u8>,
}

impl FilesystemNativeHandleFinalPathQueryReplayRecord {
    /// Validates and records one final-path query lane for a bounded
    /// chain.
    ///
    /// `buffer_resolution` must equal `buffer_pre_bytes` (the resolved
    /// snapshot precedes the call), `buffer_post_bytes` must hold exactly
    /// `returned_path` followed by its NUL terminator and an unchanged
    /// tail, `capacity` must fit the carrier with room for the terminator,
    /// and `result` must equal the returned path length.
    pub fn new(
        capacity: u64,
        flags: u32,
        result: i64,
        post_error: i32,
        buffer_resolution: Vec<u8>,
        buffer_pre_bytes: Vec<u8>,
        buffer_post_bytes: Vec<u8>,
        returned_path: Vec<u8>,
    ) -> Result<Self, String> {
        if returned_path.is_empty() {
            return Err(
                "filesystem native-handle final-path query requires a returned path".to_owned(),
            );
        }
        if i64::try_from(returned_path.len())
            .map(|length| length != result)
            .unwrap_or(true)
        {
            return Err(
                "filesystem native-handle final-path query result must be the returned path length"
                    .to_owned(),
            );
        }
        let Ok(capacity) = usize::try_from(capacity) else {
            return Err(
                "filesystem native-handle final-path query capacity must fit the host".to_owned(),
            );
        };
        if capacity > buffer_post_bytes.len() || returned_path.len() >= capacity {
            return Err(
                "filesystem native-handle final-path query capacity must hold the path terminator"
                    .to_owned(),
            );
        }
        if buffer_resolution != buffer_pre_bytes
            || buffer_pre_bytes.len() != buffer_post_bytes.len()
        {
            return Err(
                "filesystem native-handle final-path query buffer custody is inconsistent"
                    .to_owned(),
            );
        }
        if buffer_post_bytes[..returned_path.len()] != returned_path[..]
            || buffer_post_bytes[returned_path.len()] != 0
            || buffer_post_bytes[returned_path.len() + 1..]
                != buffer_pre_bytes[returned_path.len() + 1..]
        {
            return Err(
                "filesystem native-handle final-path query post state must carry the returned path"
                    .to_owned(),
            );
        }
        Ok(Self {
            capacity: capacity as u64,
            flags,
            result,
            post_error,
            buffer_resolution,
            buffer_pre_bytes,
            buffer_post_bytes,
            returned_path,
        })
    }
}

/// One handle-free `get_last_error` error-slot read inside a bounded
/// query-release chain.
///
/// The returned value equals the thread error slot, so the attempt result
/// is the read error and the recorded post-error is the same value: the
/// read observes but does not clear the slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemNativeHandleErrorObservationReplayRecord {
    error: i32,
}

impl FilesystemNativeHandleErrorObservationReplayRecord {
    /// Records one `get_last_error` read of `error`.
    pub const fn new(error: i32) -> Self {
        Self { error }
    }
}

/// One bounded intervening operation on a constrained native-handle
/// query-release chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FilesystemNativeHandleQueryOperationReplayRecord {
    FinalPathName(FilesystemNativeHandleFinalPathQueryReplayRecord),
    LastError(FilesystemNativeHandleErrorObservationReplayRecord),
}

/// One bounded Source native-handle query-release lifecycle.
///
/// The record commits to the exact acquisition contract, the exact logical
/// identity, every intervening observation's exact lanes, and the exact
/// successful release that retires the identity. Rebuilding the record
/// from observed attempts requires
/// `source_native_handle_query_chain_is_exact`, which rejects failed
/// acquisition, deferred-deletion acquisition flags, borrowed or
/// substituted handle inputs, intervening retirement, missing or
/// ambiguous release, and any lane the family does not retain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemSourceNativeHandleQueryChainReplayRecord {
    pub(crate) source_root: FilesystemGrantRootIdentity,
    pub(crate) source_relative_path: Vec<u8>,
    pub(crate) logical_handle_identity: FilesystemLogicalHandleIdentity,
    open_post_error: i32,
    operations: Vec<FilesystemNativeHandleQueryOperationReplayRecord>,
    close_result: i64,
    close_post_error: i32,
}

impl FilesystemSourceNativeHandleQueryChainReplayRecord {
    /// Validates and records one constrained Source `open_path_handle` /
    /// observation chain / `close_handle` lifecycle.
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
        logical_handle_identity: u64,
        open_post_error: i32,
        operations: Vec<FilesystemNativeHandleQueryOperationReplayRecord>,
        close_result: i64,
        close_post_error: i32,
    ) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| {
                "filesystem replay Source native-handle query chain requires a handle identity"
                    .to_owned()
            })?;
        if !filesystem_root_relative_path_is_canonical(&source_relative_path, false) {
            return Err(
                "filesystem replay Source native-handle query chain requires a canonical relative path"
                    .to_owned(),
            );
        }
        if operations.is_empty()
            || !operations.iter().any(|operation| {
                matches!(
                    operation,
                    FilesystemNativeHandleQueryOperationReplayRecord::FinalPathName(_)
                )
            })
            || operations.len().checked_add(2).is_none()
        {
            return Err(
                "filesystem replay Source native-handle query chain requires at least one final-path observation"
                    .to_owned(),
            );
        }
        if close_result == 0 {
            return Err(
                "filesystem replay Source native-handle query chain requires a successful close"
                    .to_owned(),
            );
        }
        Ok(Self {
            source_root,
            source_relative_path,
            logical_handle_identity,
            open_post_error,
            operations,
            close_result,
            close_post_error,
        })
    }

    /// Logical identity whose constrained lifecycle this record binds.
    pub const fn logical_handle_identity(&self) -> FilesystemLogicalHandleIdentity {
        self.logical_handle_identity
    }

    /// Attempt count this record rehydrates (open, operations, close).
    pub fn attempt_count(&self) -> usize {
        self.operations.len() + 2
    }
}

/// Rehydrates the exact attempt sequence retained by a query-release
/// chain record.
pub(crate) fn source_native_handle_query_chain_attempts(
    record: FilesystemSourceNativeHandleQueryChainReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let mut attempts = Vec::with_capacity(record.attempt_count());
    attempts.push(source_native_handle_query_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    ));
    for operation in record.operations {
        attempts.push(match operation {
            FilesystemNativeHandleQueryOperationReplayRecord::FinalPathName(query) => {
                native_handle_final_path_query_attempt(identity, query)
            }
            FilesystemNativeHandleQueryOperationReplayRecord::LastError(observation) => {
                native_handle_error_observation_attempt(observation)
            }
        });
    }
    attempts.push(source_native_handle_close_attempt(
        identity,
        record.close_result,
        record.close_post_error,
    ));
    attempts
}

fn source_native_handle_query_open_attempt(
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: OPEN_PATH_HANDLE_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error,
        }),
        scalar_operands: vec![
            FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::U32(QUERY_OPEN_DESIRED_ACCESS),
            },
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::U32(QUERY_OPEN_SHARE_MODE),
            },
            FilesystemScalarOperand {
                operand_ordinal: 3,
                value: FilesystemScalarOperandValue::I64(QUERY_OPEN_SECURITY_ATTRIBUTES),
            },
            FilesystemScalarOperand {
                operand_ordinal: 4,
                value: FilesystemScalarOperandValue::U32(QUERY_OPEN_CREATION_DISPOSITION),
            },
            FilesystemScalarOperand {
                operand_ordinal: 5,
                value: FilesystemScalarOperandValue::U32(QUERY_OPEN_FLAGS_AND_ATTRIBUTES),
            },
        ],
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
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 6,
            kind: FilesystemLogicalHandleKind::Native,
            resolution: FilesystemLogicalHandleInputResolution::Null,
        }],
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Native,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn native_handle_final_path_query_attempt(
    identity: FilesystemLogicalHandleIdentity,
    record: FilesystemNativeHandleFinalPathQueryReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(record.result),
            post_error: record.post_error,
        }),
        scalar_operands: vec![
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::U64(record.capacity),
            },
            FilesystemScalarOperand {
                operand_ordinal: 3,
                value: FilesystemScalarOperandValue::U32(record.flags),
            },
        ],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: vec![FilesystemReturnedPath {
            operand_ordinal: 1,
            kind: FilesystemReturnedPathKind::FinalPath,
            completeness: FilesystemReturnedPathCompleteness::Complete,
            bytes: record.returned_path,
        }],
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.buffer_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.buffer_pre_bytes,
            post_bytes: record.buffer_post_bytes,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Native,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn native_handle_error_observation_attempt(
    record: FilesystemNativeHandleErrorObservationReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: GET_LAST_ERROR_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(i64::from(record.error)),
            post_error: record.error,
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
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn source_native_handle_close_attempt(
    identity: FilesystemLogicalHandleIdentity,
    result: i64,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: CLOSE_HANDLE_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(result),
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
            kind: FilesystemLogicalHandleKind::Native,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![identity],
        grant_refusals: Vec::new(),
    }
}

/// Whether `attempts` are one exact bounded Source native-handle
/// query-release chain: constrained open, one or more admitted
/// observations including at least one final-path query, and one
/// successful close that retires the acquired identity.
pub(crate) fn source_native_handle_query_chain_is_exact<
    T: std::borrow::Borrow<FilesystemOperationAttempt>,
>(
    attempts: &[T],
) -> bool {
    if attempts.len() < 3 {
        return false;
    }
    let Some(identity) = source_native_handle_query_open_identity(attempts[0].borrow()) else {
        return false;
    };
    let middle = &attempts[1..attempts.len() - 1];
    middle.iter().any(|operation| {
        operation.borrow().operation_tag == FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG
    }) && middle
        .iter()
        .all(|operation| native_handle_query_operation_is_exact(operation.borrow(), identity))
        && source_native_handle_close_is_exact(attempts[attempts.len() - 1].borrow(), identity)
}

/// Identity acquired by one constrained Source `open_path_handle`, or
/// `None` when the attempt is not the exact contract.
fn source_native_handle_query_open_identity(
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
            value: FilesystemScalarOperandValue::U32(QUERY_OPEN_DESIRED_ACCESS),
        },
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U32(QUERY_OPEN_SHARE_MODE),
        },
        FilesystemScalarOperand {
            operand_ordinal: 3,
            value: FilesystemScalarOperandValue::I64(QUERY_OPEN_SECURITY_ATTRIBUTES),
        },
        FilesystemScalarOperand {
            operand_ordinal: 4,
            value: FilesystemScalarOperandValue::U32(QUERY_OPEN_CREATION_DISPOSITION),
        },
        FilesystemScalarOperand {
            operand_ordinal: 5,
            value: FilesystemScalarOperandValue::U32(QUERY_OPEN_FLAGS_AND_ATTRIBUTES),
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
    let [template] = open.logical_handle_inputs.as_slice() else {
        return None;
    };
    let output = open.logical_handle_output?;
    (open.operation_tag == OPEN_PATH_HANDLE_OPERATION_TAG
        && open.provider == FilesystemObservationProvider::RealScoped
        && rooted.operand_ordinal == 0
        && filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        && authorized.operand_ordinal == 0
        && authorized.access == FilesystemGrantAccess::Read
        && authorized.root == rooted.root
        && authorized.relative_path == rooted.relative_path
        && template.operand_ordinal == 6
        && template.kind == FilesystemLogicalHandleKind::Native
        && template.resolution == FilesystemLogicalHandleInputResolution::Null
        && output.kind == FilesystemLogicalHandleKind::Native
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
        && open.retired_logical_handles.is_empty()
        && open.grant_refusals.is_empty())
    .then_some(identity)
}

fn native_handle_query_operation_is_exact(
    operation: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> bool {
    match operation.operation_tag {
        FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG => {
            native_handle_final_path_query_is_exact(operation, identity)
        }
        GET_LAST_ERROR_OPERATION_TAG => native_handle_error_observation_is_exact(operation),
        _ => false,
    }
}

fn native_handle_final_path_query_is_exact(
    query: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> bool {
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(result),
        post_error,
    }) = query.outcome
    else {
        return false;
    };
    let [
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(capacity),
        },
        FilesystemScalarOperand {
            operand_ordinal: 3,
            value: FilesystemScalarOperandValue::U32(flags),
        },
    ] = query.scalar_operands.as_slice()
    else {
        return false;
    };
    let [returned] = query.returned_paths.as_slice() else {
        return false;
    };
    let [resolution] = query.mutable_byte_operand_resolutions.as_slice() else {
        return false;
    };
    let [mutable] = query.mutable_byte_operands.as_slice() else {
        return false;
    };
    let [handle] = query.logical_handle_inputs.as_slice() else {
        return false;
    };
    query.provider == FilesystemObservationProvider::RealScoped
        && returned.operand_ordinal == 1
        && returned.kind == FilesystemReturnedPathKind::FinalPath
        && returned.completeness == FilesystemReturnedPathCompleteness::Complete
        && resolution.operand_ordinal == 1
        && mutable.operand_ordinal == 1
        && handle.operand_ordinal == 0
        && handle.kind == FilesystemLogicalHandleKind::Native
        && handle.resolution == FilesystemLogicalHandleInputResolution::Resolved(identity)
        && FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            *capacity,
            *flags,
            result,
            post_error,
            resolution.bytes.clone(),
            mutable.pre_bytes.clone(),
            mutable.post_bytes.clone(),
            returned.bytes.clone(),
        )
        .is_ok()
        && query.byte_operands.is_empty()
        && query.path_like_operands.is_empty()
        && query.rooted_path_operand_resolutions.is_empty()
        && query.observed_byte_regions.is_empty()
        && query.metadata_observations.is_empty()
        && query.mutable_i64_operand_resolutions.is_empty()
        && query.mutable_i64_operands.is_empty()
        && query.authorized_paths.is_empty()
        && query.logical_handle_output.is_none()
        && query.retired_logical_handles.is_empty()
        && query.grant_refusals.is_empty()
}

fn native_handle_error_observation_is_exact(operation: &FilesystemOperationAttempt) -> bool {
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(result),
        post_error,
    }) = operation.outcome
    else {
        return false;
    };
    operation.provider == FilesystemObservationProvider::RealScoped
        && result == i64::from(post_error)
        && operation.scalar_operands.is_empty()
        && operation.byte_operands.is_empty()
        && operation.path_like_operands.is_empty()
        && operation.rooted_path_operand_resolutions.is_empty()
        && operation.returned_paths.is_empty()
        && operation.observed_byte_regions.is_empty()
        && operation.metadata_observations.is_empty()
        && operation.mutable_byte_operand_resolutions.is_empty()
        && operation.mutable_i64_operand_resolutions.is_empty()
        && operation.mutable_byte_operands.is_empty()
        && operation.mutable_i64_operands.is_empty()
        && operation.authorized_paths.is_empty()
        && operation.logical_handle_inputs.is_empty()
        && operation.logical_handle_output.is_none()
        && operation.retired_logical_handles.is_empty()
        && operation.grant_refusals.is_empty()
}

fn source_native_handle_close_is_exact(
    close: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
) -> bool {
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(result),
        ..
    }) = close.outcome
    else {
        return false;
    };
    let [handle] = close.logical_handle_inputs.as_slice() else {
        return false;
    };
    close.operation_tag == CLOSE_HANDLE_OPERATION_TAG
        && close.provider == FilesystemObservationProvider::RealScoped
        && result != 0
        && handle.operand_ordinal == 0
        && handle.kind == FilesystemLogicalHandleKind::Native
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
