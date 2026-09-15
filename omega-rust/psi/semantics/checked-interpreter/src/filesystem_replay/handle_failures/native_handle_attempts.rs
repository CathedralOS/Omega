//! Exact native handle attempts: get-osfhandle, close and final path name.

use crate::filesystem_replay::handle_failures::descriptor_attempts::{
    unknown_descriptor_failure_attempt,
    unknown_descriptor_failure_has_exact_core_shape_with_outcome,
    unknown_handle_failure_has_exact_core_shape_with_outcome,
};
use crate::filesystem_replay::handle_failures::{
    CLOSE_HANDLE_OPERATION_TAG, FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG,
    GET_OSF_HANDLE_OPERATION_TAG, INVALID_HANDLE_ERROR, UNCHANGED_ERROR,
    UNKNOWN_DESCRIPTOR_OSF_HANDLE_RESULT, UNKNOWN_NATIVE_HANDLE_CLOSE_RESULT,
    UNKNOWN_NATIVE_HANDLE_FINAL_PATH_RESULT,
};
use crate::{
    FilesystemLogicalHandleKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemScalarOperand,
    FilesystemScalarOperandValue,
};

pub(crate) fn unknown_descriptor_get_osfhandle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape_with_outcome(
            attempt,
            GET_OSF_HANDLE_OPERATION_TAG,
            UNKNOWN_DESCRIPTOR_OSF_HANDLE_RESULT,
            UNCHANGED_ERROR,
        )
}

pub(crate) fn unknown_descriptor_get_osfhandle_attempt() -> FilesystemOperationAttempt {
    let mut attempt = unknown_descriptor_failure_attempt(GET_OSF_HANDLE_OPERATION_TAG, Vec::new());
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_OSF_HANDLE_RESULT),
        post_error: UNCHANGED_ERROR,
    });
    attempt
}

pub(crate) fn unknown_native_handle_close_handle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_handle_failure_has_exact_core_shape_with_outcome(
            attempt,
            CLOSE_HANDLE_OPERATION_TAG,
            UNKNOWN_NATIVE_HANDLE_CLOSE_RESULT,
            INVALID_HANDLE_ERROR,
            FilesystemLogicalHandleKind::Native,
        )
}

pub(crate) fn unknown_native_handle_close_handle_attempt() -> FilesystemOperationAttempt {
    let mut attempt = unknown_descriptor_failure_attempt(CLOSE_HANDLE_OPERATION_TAG, Vec::new());
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(UNKNOWN_NATIVE_HANDLE_CLOSE_RESULT),
        post_error: INVALID_HANDLE_ERROR,
    });
    attempt.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    attempt
}

pub(crate) fn unknown_native_handle_final_path_name_by_handle_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(&[u8], u64, u32)> {
    let [capacity, flags] = attempt.scalar_operands.as_slice() else {
        return None;
    };
    let (
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(capacity),
        },
        FilesystemScalarOperand {
            operand_ordinal: 3,
            value: FilesystemScalarOperandValue::U32(flags),
        },
    ) = (capacity, flags)
    else {
        return None;
    };
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    let capacity_on_host = usize::try_from(*capacity).ok()?;
    (attempt.byte_operands.is_empty()
        && resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && capacity_on_host <= resolution.bytes.len()
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_handle_failure_has_exact_core_shape_with_outcome(
            attempt,
            FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG,
            UNKNOWN_NATIVE_HANDLE_FINAL_PATH_RESULT,
            INVALID_HANDLE_ERROR,
            FilesystemLogicalHandleKind::Native,
        ))
    .then_some((resolution.bytes.as_slice(), *capacity, *flags))
}

pub(crate) fn unknown_native_handle_final_path_name_by_handle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_native_handle_final_path_name_by_handle_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_native_handle_final_path_name_by_handle_attempt(
    buffer: Vec<u8>,
    capacity: u64,
    flags: u32,
) -> FilesystemOperationAttempt {
    let resolution_buffer = buffer.clone();
    let pre_buffer = buffer.clone();
    let mut attempt = unknown_descriptor_failure_attempt(
        FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG,
        vec![
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::U64(capacity),
            },
            FilesystemScalarOperand {
                operand_ordinal: 3,
                value: FilesystemScalarOperandValue::U32(flags),
            },
        ],
    );
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(UNKNOWN_NATIVE_HANDLE_FINAL_PATH_RESULT),
        post_error: INVALID_HANDLE_ERROR,
    });
    attempt.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
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
