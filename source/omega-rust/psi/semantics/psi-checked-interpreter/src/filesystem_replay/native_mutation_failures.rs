use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FilesystemByteOperand, FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemObservationProvider,
    FilesystemOperationAttempt, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemReplay, FilesystemScalarOperand, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayRecord, source_input_record_attempts,
    validate_filesystem_replay_size, validate_source_input_attempts,
};

const SET_FILE_TIME_OPERATION_TAG: u16 = 32;
const LOCK_FILE_EX_OPERATION_TAG: u16 = 33;
const UNLOCK_FILE_OPERATION_TAG: u16 = 34;
const INVALID_HANDLE_RESULT: i64 = 0;
const INVALID_HANDLE_ERROR: i32 = 6;
const FILE_TIME_BYTES: usize = 8;
const OVERLAPPED_BYTES: usize = 32;

/// One closed failed mutation on an unknown compiler-owned synthetic native
/// handle. Each variant owns exactly the authored operands retained by replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemInputUnknownNativeHandleMutationReplayKind {
    SetFileTime {
        creation: i64,
        last_access: Vec<u8>,
        last_write: Vec<u8>,
    },
    LockFileEx {
        flags: u32,
        reserved: u32,
        length_low: u32,
        length_high: u32,
        overlapped: Vec<u8>,
    },
    UnlockFile {
        offset_low: u32,
        offset_high: u32,
        length_low: u32,
        length_high: u32,
    },
}

/// Optional exact Source-input prefix followed by one selected failed native
/// mutation on an unknown compiler-owned synthetic handle.
///
/// The fixed result and error describe Omega evaluator behavior only. This
/// record retains no operating-system handle or native authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownNativeHandleMutationReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownNativeHandleMutationReplayKind,
}

impl FilesystemInputUnknownNativeHandleMutationReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownNativeHandleMutationReplayKind,
    ) -> Result<Self, String> {
        validate_typed_kind(&kind)?;
        Ok(Self { source_input, kind })
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> &FilesystemInputUnknownNativeHandleMutationReplayKind {
        &self.kind
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownNativeHandleMutationReplayKind,
    ) {
        (self.source_input, self.kind)
    }
}

impl FilesystemReplay {
    /// Construct optional exact Source input followed by one typed unknown
    /// synthetic-native-handle mutation failure.
    pub fn from_input_unknown_native_handle_mutation_record(
        record: FilesystemInputUnknownNativeHandleMutationReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        attempts.push(unknown_native_handle_mutation_attempt(kind));
        validate_filesystem_replay_size(&attempts)?;
        validate_native_mutation_attempts(&attempts, &[])?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate observations containing optional exact Source input followed
    /// by exactly one selected unknown synthetic-native-handle mutation failure.
    pub fn from_input_unknown_native_handle_mutation_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        validate_native_mutation_attempts(attempts, observations.build_included_sources())?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}

fn validate_typed_kind(
    kind: &FilesystemInputUnknownNativeHandleMutationReplayKind,
) -> Result<(), String> {
    match kind {
        FilesystemInputUnknownNativeHandleMutationReplayKind::SetFileTime {
            last_access,
            last_write,
            ..
        } if last_access.len() < FILE_TIME_BYTES || last_write.len() < FILE_TIME_BYTES => {
            Err(format!(
                "filesystem replay set_file_time carriers must each retain at least {FILE_TIME_BYTES} bytes"
            ))
        }
        FilesystemInputUnknownNativeHandleMutationReplayKind::LockFileEx { overlapped, .. }
            if overlapped.len() < OVERLAPPED_BYTES =>
        {
            Err(format!(
                "filesystem replay lock_file_ex carrier must retain at least {OVERLAPPED_BYTES} bytes"
            ))
        }
        _ => Ok(()),
    }
}

fn validate_native_mutation_attempts(
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failed unknown-native-handle mutation cannot hand off generated sources"
                .to_owned(),
        );
    }
    let (operation, source_attempts) = attempts.split_last().ok_or_else(|| {
        "filesystem replay requires one failed unknown-native-handle mutation".to_owned()
    })?;
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if !unknown_native_handle_mutation_attempt_is_exact(operation) {
        return Err(
            "filesystem replay failed unknown-native-handle mutation lanes are inconsistent"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn unknown_native_handle_mutation_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<FilesystemInputUnknownNativeHandleMutationReplayKind> {
    match exact_native_mutation_operands(attempt)? {
        ExactNativeMutationOperands::SetFileTime {
            creation,
            last_access,
            last_write,
        } => Some(
            FilesystemInputUnknownNativeHandleMutationReplayKind::SetFileTime {
                creation,
                last_access: last_access.to_vec(),
                last_write: last_write.to_vec(),
            },
        ),
        ExactNativeMutationOperands::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped,
        } => Some(
            FilesystemInputUnknownNativeHandleMutationReplayKind::LockFileEx {
                flags,
                reserved,
                length_low,
                length_high,
                overlapped: overlapped.to_vec(),
            },
        ),
        ExactNativeMutationOperands::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        } => Some(
            FilesystemInputUnknownNativeHandleMutationReplayKind::UnlockFile {
                offset_low,
                offset_high,
                length_low,
                length_high,
            },
        ),
    }
}

enum ExactNativeMutationOperands<'a> {
    SetFileTime {
        creation: i64,
        last_access: &'a [u8],
        last_write: &'a [u8],
    },
    LockFileEx {
        flags: u32,
        reserved: u32,
        length_low: u32,
        length_high: u32,
        overlapped: &'a [u8],
    },
    UnlockFile {
        offset_low: u32,
        offset_high: u32,
        length_low: u32,
        length_high: u32,
    },
}

fn exact_native_mutation_operands(
    attempt: &FilesystemOperationAttempt,
) -> Option<ExactNativeMutationOperands<'_>> {
    if !unknown_native_handle_mutation_has_exact_core(attempt) {
        return None;
    }
    match attempt.operation_tag {
        SET_FILE_TIME_OPERATION_TAG => {
            let [creation] = attempt.scalar_operands.as_slice() else {
                return None;
            };
            let FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I64(creation),
            } = creation
            else {
                return None;
            };
            let [last_access, last_write] = attempt.byte_operands.as_slice() else {
                return None;
            };
            (attempt.mutable_byte_operand_resolutions.is_empty()
                && attempt.mutable_byte_operands.is_empty()
                && last_access.operand_ordinal == 2
                && last_access.bytes.len() >= FILE_TIME_BYTES
                && last_write.operand_ordinal == 3
                && last_write.bytes.len() >= FILE_TIME_BYTES)
                .then_some(ExactNativeMutationOperands::SetFileTime {
                    creation: *creation,
                    last_access: &last_access.bytes,
                    last_write: &last_write.bytes,
                })
        }
        LOCK_FILE_EX_OPERATION_TAG => {
            let [flags, reserved, length_low, length_high] = attempt.scalar_operands.as_slice()
            else {
                return None;
            };
            let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
                return None;
            };
            let [carrier] = attempt.mutable_byte_operands.as_slice() else {
                return None;
            };
            let values =
                exact_u32_operands([flags, reserved, length_low, length_high], [1, 2, 3, 4])?;
            (attempt.byte_operands.is_empty()
                && resolution.operand_ordinal == 5
                && carrier.operand_ordinal == 5
                && resolution.bytes.len() >= OVERLAPPED_BYTES
                && resolution.bytes == carrier.pre_bytes
                && resolution.bytes == carrier.post_bytes)
                .then_some(ExactNativeMutationOperands::LockFileEx {
                    flags: values[0],
                    reserved: values[1],
                    length_low: values[2],
                    length_high: values[3],
                    overlapped: &resolution.bytes,
                })
        }
        UNLOCK_FILE_OPERATION_TAG => {
            let [offset_low, offset_high, length_low, length_high] =
                attempt.scalar_operands.as_slice()
            else {
                return None;
            };
            let values = exact_u32_operands(
                [offset_low, offset_high, length_low, length_high],
                [1, 2, 3, 4],
            )?;
            (attempt.byte_operands.is_empty()
                && attempt.mutable_byte_operand_resolutions.is_empty()
                && attempt.mutable_byte_operands.is_empty())
            .then_some(ExactNativeMutationOperands::UnlockFile {
                offset_low: values[0],
                offset_high: values[1],
                length_low: values[2],
                length_high: values[3],
            })
        }
        _ => None,
    }
}

fn exact_u32_operands(
    operands: [&FilesystemScalarOperand; 4],
    ordinals: [u8; 4],
) -> Option<[u32; 4]> {
    let mut values = [0; 4];
    for (index, operand) in operands.into_iter().enumerate() {
        let FilesystemScalarOperand {
            operand_ordinal,
            value: FilesystemScalarOperandValue::U32(value),
        } = operand
        else {
            return None;
        };
        if *operand_ordinal != ordinals[index] {
            return None;
        }
        values[index] = *value;
    }
    Some(values)
}

pub(super) fn unknown_native_handle_mutation_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    match exact_native_mutation_operands(attempt) {
        Some(ExactNativeMutationOperands::SetFileTime {
            creation,
            last_access,
            last_write,
        }) => {
            let _ = (creation, last_access, last_write);
            true
        }
        Some(ExactNativeMutationOperands::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped,
        }) => {
            let _ = (flags, reserved, length_low, length_high, overlapped);
            true
        }
        Some(ExactNativeMutationOperands::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        }) => {
            let _ = (offset_low, offset_high, length_low, length_high);
            true
        }
        None => false,
    }
}

fn unknown_native_handle_mutation_has_exact_core(attempt: &FilesystemOperationAttempt) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: SET_FILE_TIME_OPERATION_TAG
                | LOCK_FILE_EX_OPERATION_TAG
                | UNLOCK_FILE_OPERATION_TAG,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(INVALID_HANDLE_RESULT),
                post_error: INVALID_HANDLE_ERROR,
            }),
            scalar_operands: _,
            byte_operands: _,
            path_like_operands,
            rooted_path_operand_resolutions,
            returned_paths,
            observed_byte_regions,
            metadata_observations,
            mutable_byte_operand_resolutions: _,
            mutable_i64_operand_resolutions,
            mutable_byte_operands: _,
            mutable_i64_operands,
            authorized_paths,
            logical_handle_inputs,
            logical_handle_output: None,
            retired_logical_handles,
            grant_refusals,
        } if path_like_operands.is_empty()
            && rooted_path_operand_resolutions.is_empty()
            && returned_paths.is_empty()
            && observed_byte_regions.is_empty()
            && metadata_observations.is_empty()
            && mutable_i64_operand_resolutions.is_empty()
            && mutable_i64_operands.is_empty()
            && authorized_paths.is_empty()
            && matches!(
                logical_handle_inputs.as_slice(),
                [FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Native,
                    resolution: FilesystemLogicalHandleInputResolution::Unknown,
                }]
            )
            && retired_logical_handles.is_empty()
            && grant_refusals.is_empty()
    )
}

pub(super) fn unknown_native_handle_mutation_attempt(
    kind: FilesystemInputUnknownNativeHandleMutationReplayKind,
) -> FilesystemOperationAttempt {
    let (operation_tag, scalar_operands, byte_operands, resolutions, mutable_operands) = match kind
    {
        FilesystemInputUnknownNativeHandleMutationReplayKind::SetFileTime {
            creation,
            last_access,
            last_write,
        } => (
            SET_FILE_TIME_OPERATION_TAG,
            vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I64(creation),
            }],
            vec![
                FilesystemByteOperand {
                    operand_ordinal: 2,
                    bytes: last_access,
                },
                FilesystemByteOperand {
                    operand_ordinal: 3,
                    bytes: last_write,
                },
            ],
            Vec::new(),
            Vec::new(),
        ),
        FilesystemInputUnknownNativeHandleMutationReplayKind::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped,
        } => {
            let resolution = overlapped.clone();
            let pre = overlapped.clone();
            (
                LOCK_FILE_EX_OPERATION_TAG,
                u32_operands([flags, reserved, length_low, length_high]),
                Vec::new(),
                vec![FilesystemMutableByteOperandResolution {
                    operand_ordinal: 5,
                    bytes: resolution,
                }],
                vec![FilesystemMutableByteOperand {
                    operand_ordinal: 5,
                    pre_bytes: pre,
                    post_bytes: overlapped,
                }],
            )
        }
        FilesystemInputUnknownNativeHandleMutationReplayKind::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        } => (
            UNLOCK_FILE_OPERATION_TAG,
            u32_operands([offset_low, offset_high, length_low, length_high]),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    };
    FilesystemOperationAttempt {
        operation_tag,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(INVALID_HANDLE_RESULT),
            post_error: INVALID_HANDLE_ERROR,
        }),
        scalar_operands,
        byte_operands,
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: resolutions,
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: mutable_operands,
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Native,
            resolution: FilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn u32_operands(values: [u32; 4]) -> Vec<FilesystemScalarOperand> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| FilesystemScalarOperand {
            operand_ordinal: u8::try_from(index + 1).expect("four operands fit u8"),
            value: FilesystemScalarOperandValue::U32(value),
        })
        .collect()
}
