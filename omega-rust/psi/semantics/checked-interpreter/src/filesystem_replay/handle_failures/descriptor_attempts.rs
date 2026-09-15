//! Exact descriptor attempts: seek, read, write, metadata and file times.

use crate::filesystem_replay::handle_failures::handle_replays::unknown_descriptor_operation_attempt_is_exact;
use crate::filesystem_replay::handle_failures::native_handle_attempts::{
    unknown_descriptor_get_osfhandle_attempt_is_exact,
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
};
use crate::filesystem_replay::handle_failures::{
    BAD_DESCRIPTOR_ERROR, FilesystemInputUnknownDescriptorReadReplayKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    FilesystemInputUnknownDescriptorWriteReplayKind, READ_AT_OPERATION_TAG,
    READ_FILE_METADATA_OPERATION_TAG, READ_OPERATION_TAG, SEEK_OPERATION_TAG,
    SET_FILE_TIMES_MINIMUM_CARRIER_BYTES, SET_FILE_TIMES_OPERATION_TAG, UNKNOWN_DESCRIPTOR_RESULT,
    WRITE_AT_OPERATION_TAG, WRITE_OPERATION_TAG,
};
use crate::{
    FILESYSTEM_METADATA_API_CARRIER_BYTES, FilesystemByteOperand, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemScalarOperand, FilesystemScalarOperandValue,
};

pub(crate) fn unknown_descriptor_seek_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(i64, i32)> {
    let [offset, whence] = attempt.scalar_operands.as_slice() else {
        return None;
    };
    let (
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I64(offset),
        },
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::I32(whence),
        },
    ) = (offset, whence)
    else {
        return None;
    };
    unknown_descriptor_failure_has_exact_common_shape(attempt, SEEK_OPERATION_TAG)
        .then_some((*offset, *whence))
}

pub(crate) fn unknown_descriptor_seek_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_seek_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_seek_attempt(
    offset: i64,
    whence: i32,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(
        SEEK_OPERATION_TAG,
        vec![
            FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I64(offset),
            },
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I32(whence),
            },
        ],
    )
}

pub(crate) fn unknown_descriptor_write_operation_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<FilesystemInputUnknownDescriptorWriteOperationReplayKind> {
    let kind = match (attempt.operation_tag, attempt.scalar_operands.as_slice()) {
        (
            17,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::U32(mode),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteOperationReplayKind::SetFilePermissions {
            mode: *mode,
        },
        (
            41,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I64(length),
                },
            ],
        ) => {
            FilesystemInputUnknownDescriptorWriteOperationReplayKind::SetLength { length: *length }
        }
        (
            46,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I32(operation),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteOperationReplayKind::LockFile {
            operation: *operation,
        },
        (
            49,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I32(uid),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I32(gid),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteOperationReplayKind::ChangeFileOwner {
            uid: *uid,
            gid: *gid,
        },
        _ => return None,
    };
    unknown_descriptor_failure_has_exact_common_shape(attempt, kind.operation_tag()).then_some(kind)
}

pub(crate) fn unknown_descriptor_read_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(FilesystemInputUnknownDescriptorReadReplayKind, &[u8])> {
    let kind = match (attempt.operation_tag, attempt.scalar_operands.as_slice()) {
        (
            READ_OPERATION_TAG,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(count),
                },
            ],
        ) => FilesystemInputUnknownDescriptorReadReplayKind::Sequential { count: *count },
        (
            READ_AT_OPERATION_TAG,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(count),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 3,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
            ],
        ) => FilesystemInputUnknownDescriptorReadReplayKind::Positioned {
            count: *count,
            offset: *offset,
        },
        _ => return None,
    };
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    let count = usize::try_from(kind.count()).ok()?;
    (resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && count <= resolution.bytes.len()
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_descriptor_failure_has_exact_base_shape(attempt, kind.operation_tag()))
    .then_some((kind, resolution.bytes.as_slice()))
}

pub(crate) fn unknown_descriptor_read_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_read_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_read_attempt(
    kind: FilesystemInputUnknownDescriptorReadReplayKind,
    buffer: Vec<u8>,
) -> FilesystemOperationAttempt {
    let resolution_buffer = buffer.clone();
    let pre_buffer = buffer.clone();
    let mut attempt =
        unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands());
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_buffer,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_buffer,
        post_bytes: buffer,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_read_file_metadata_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<&[u8]> {
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    (attempt.scalar_operands.is_empty()
        && resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && resolution.bytes.len() >= FILESYSTEM_METADATA_API_CARRIER_BYTES
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_descriptor_failure_has_exact_base_shape(
            attempt,
            READ_FILE_METADATA_OPERATION_TAG,
        ))
    .then_some(resolution.bytes.as_slice())
}

pub(crate) fn unknown_descriptor_read_file_metadata_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_read_file_metadata_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_read_file_metadata_attempt(
    carrier: Vec<u8>,
) -> FilesystemOperationAttempt {
    let resolution_carrier = carrier.clone();
    let pre_carrier = carrier.clone();
    let mut attempt =
        unknown_descriptor_failure_attempt(READ_FILE_METADATA_OPERATION_TAG, Vec::new());
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_carrier,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_carrier,
        post_bytes: carrier,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_write_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(FilesystemInputUnknownDescriptorWriteReplayKind, &[u8])> {
    let kind = match (attempt.operation_tag, attempt.scalar_operands.as_slice()) {
        (WRITE_OPERATION_TAG, []) => FilesystemInputUnknownDescriptorWriteReplayKind::Sequential,
        (
            WRITE_AT_OPERATION_TAG,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteReplayKind::Positioned { offset: *offset },
        _ => return None,
    };
    let [payload] = attempt.byte_operands.as_slice() else {
        return None;
    };
    (payload.operand_ordinal == 1
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape(attempt, kind.operation_tag()))
    .then_some((kind, payload.bytes.as_slice()))
}

pub(crate) fn unknown_descriptor_write_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_write_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_write_attempt(
    kind: FilesystemInputUnknownDescriptorWriteReplayKind,
    payload: Vec<u8>,
) -> FilesystemOperationAttempt {
    let mut attempt =
        unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands());
    attempt.byte_operands = vec![FilesystemByteOperand {
        operand_ordinal: 1,
        bytes: payload,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_write_operation_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_write_operation_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_write_operation_attempt(
    kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands())
}

pub(crate) fn unknown_input_handle_failure_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_operation_attempt_is_exact(attempt)
        || unknown_native_handle_close_handle_attempt_is_exact(attempt)
        || unknown_native_handle_final_path_name_by_handle_attempt_is_exact(attempt)
        || unknown_descriptor_get_osfhandle_attempt_is_exact(attempt)
        || unknown_descriptor_seek_attempt_is_exact(attempt)
        || unknown_descriptor_read_attempt_is_exact(attempt)
        || unknown_descriptor_read_file_metadata_attempt_is_exact(attempt)
        || unknown_descriptor_write_attempt_is_exact(attempt)
        || unknown_descriptor_write_operation_attempt_is_exact(attempt)
        || unknown_descriptor_set_file_times_attempt_is_exact(attempt)
}

pub(crate) fn unknown_descriptor_failure_has_exact_common_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    unknown_descriptor_failure_has_exact_base_shape(attempt, operation_tag)
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
}

fn unknown_descriptor_failure_has_exact_base_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    attempt.byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape(attempt, operation_tag)
}

pub(crate) fn unknown_descriptor_failure_has_exact_core_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    unknown_descriptor_failure_has_exact_core_shape_with_outcome(
        attempt,
        operation_tag,
        UNKNOWN_DESCRIPTOR_RESULT,
        BAD_DESCRIPTOR_ERROR,
    )
}

pub(crate) fn unknown_descriptor_failure_has_exact_fixed_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    attempt.byte_operands.is_empty()
        && unknown_handle_failure_has_exact_fixed_shape_with_outcome(
            attempt,
            operation_tag,
            UNKNOWN_DESCRIPTOR_RESULT,
            BAD_DESCRIPTOR_ERROR,
            FilesystemLogicalHandleKind::Descriptor,
        )
}

pub(crate) fn unknown_descriptor_failure_has_exact_core_shape_with_outcome(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
    result: i64,
    post_error: i32,
) -> bool {
    unknown_handle_failure_has_exact_core_shape_with_outcome(
        attempt,
        operation_tag,
        result,
        post_error,
        FilesystemLogicalHandleKind::Descriptor,
    )
}

pub(crate) fn unknown_handle_failure_has_exact_core_shape_with_outcome(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
    result: i64,
    post_error: i32,
    logical_handle_kind: FilesystemLogicalHandleKind,
) -> bool {
    unknown_handle_failure_has_exact_fixed_shape_with_outcome(
        attempt,
        operation_tag,
        result,
        post_error,
        logical_handle_kind,
    ) && attempt.mutable_i64_operand_resolutions.is_empty()
        && attempt.mutable_i64_operands.is_empty()
}

fn unknown_handle_failure_has_exact_fixed_shape_with_outcome(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
    result: i64,
    post_error: i32,
    logical_handle_kind: FilesystemLogicalHandleKind,
) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: observed_operation_tag,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(observed_result),
                post_error: observed_post_error,
            }),
            scalar_operands: _,
            byte_operands: _,
            path_like_operands,
            rooted_path_operand_resolutions,
            returned_paths,
            observed_byte_regions,
            metadata_observations,
            mutable_byte_operand_resolutions: _,
            mutable_i64_operand_resolutions: _,
            mutable_byte_operands: _,
            mutable_i64_operands: _,
            authorized_paths,
            logical_handle_inputs,
            logical_handle_output: None,
            retired_logical_handles,
            grant_refusals,
        } if *observed_operation_tag == operation_tag
            && *observed_result == result
            && *observed_post_error == post_error
            && path_like_operands.is_empty()
            && rooted_path_operand_resolutions.is_empty()
            && returned_paths.is_empty()
            && observed_byte_regions.is_empty()
            && metadata_observations.is_empty()
            && authorized_paths.is_empty()
            && matches!(
                logical_handle_inputs.as_slice(),
                [FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind,
                    resolution: FilesystemLogicalHandleInputResolution::Unknown,
                }] if *kind == logical_handle_kind
            )
            && retired_logical_handles.is_empty()
            && grant_refusals.is_empty()
    )
}

pub(crate) fn unknown_descriptor_set_file_times_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<&[u8]> {
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    (attempt.scalar_operands.is_empty()
        && resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && resolution.bytes.len() >= SET_FILE_TIMES_MINIMUM_CARRIER_BYTES
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_descriptor_failure_has_exact_base_shape(attempt, SET_FILE_TIMES_OPERATION_TAG))
    .then_some(resolution.bytes.as_slice())
}

pub(crate) fn unknown_descriptor_set_file_times_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_set_file_times_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_set_file_times_attempt(
    times: Vec<u8>,
) -> FilesystemOperationAttempt {
    let resolution_times = times.clone();
    let pre_times = times.clone();
    let mut attempt = unknown_descriptor_failure_attempt(SET_FILE_TIMES_OPERATION_TAG, Vec::new());
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_times,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_times,
        post_bytes: times,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_failure_attempt(
    operation_tag: u16,
    scalar_operands: Vec<FilesystemScalarOperand>,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_RESULT),
            post_error: BAD_DESCRIPTOR_ERROR,
        }),
        scalar_operands,
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
            resolution: FilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}
