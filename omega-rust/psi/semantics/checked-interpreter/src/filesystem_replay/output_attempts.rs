//! Exact per-file operation decoding and canonical attempt construction.
//! `output_stream.rs` supplies borrowed operations in that file's own order;
//! unrelated files need not be adjacent in the retained execution stream.

use crate::filesystem_replay::duplicates::{
    output_duplicate_attempts, output_duplicate_record_from_attempts,
};
use crate::filesystem_replay::locks::{output_lock_attempts, output_lock_record_from_attempts};
use crate::filesystem_replay::output_ownership::{
    output_change_file_owner_attempt, output_change_file_owner_record_from_attempt,
};
use crate::filesystem_replay::replay_records::output_file_operation_attempt_count;
use crate::filesystem_replay::{
    FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE, FilesystemOutputFileOperationReplayRecord,
    FilesystemOutputFileReplayRecord, FilesystemOutputWriteReplayKind,
    FilesystemOutputWriteReplayRecord,
};
use crate::{
    FilesystemAuthorizedPath, FilesystemByteOperand, FilesystemGrantAccess,
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
    FilesystemScalarOperandValue, filesystem_root_relative_path_is_canonical,
};

pub(crate) fn output_file_record_from_attempts(
    attempts: &[&FilesystemOperationAttempt],
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
        let operation = operations[operation_cursor];
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

pub(crate) fn filesystem_output_attempt_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 1 | 9 | 11 | 12 | 19 | 20 | 27)
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
