use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    FilesystemScalarOperand, FilesystemScalarOperandValue, FilesystemSourceInputReplayRecord,
    source_input_record_attempts, validate_filesystem_replay_size, validate_source_input_attempts,
};

const UNKNOWN_DESCRIPTOR_RESULT: i64 = -1;
const BAD_DESCRIPTOR_ERROR: i32 = 9;
const SEEK_OPERATION_TAG: u16 = 10;

/// One operand-free descriptor operation whose unknown input deterministically
/// fails with `EBADF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemInputUnknownDescriptorOperationReplayKind {
    Close,
    Sync,
    SyncData,
    Duplicate,
}

impl FilesystemInputUnknownDescriptorOperationReplayKind {
    const fn operation_tag(self) -> u16 {
        match self {
            Self::Close => 8,
            Self::Sync => 43,
            Self::SyncData => 44,
            Self::Duplicate => 45,
        }
    }

    const fn from_operation_tag(operation_tag: u16) -> Option<Self> {
        match operation_tag {
            8 => Some(Self::Close),
            43 => Some(Self::Sync),
            44 => Some(Self::SyncData),
            45 => Some(Self::Duplicate),
            _ => None,
        }
    }
}

/// Optional Source-input prefix followed by exactly one operand-free operation
/// on an unknown descriptor.
///
/// The selected operation contributes no authored coordinates to this record:
/// its provider, result, error, logical input, and empty side lanes are fixed by
/// the record type. In particular, the raw provider descriptor is not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorOperationReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownDescriptorOperationReplayKind,
}

impl FilesystemInputUnknownDescriptorOperationReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownDescriptorOperationReplayKind,
    ) -> Self {
        Self { source_input, kind }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> FilesystemInputUnknownDescriptorOperationReplayKind {
        self.kind
    }

    fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownDescriptorOperationReplayKind,
    ) {
        (self.source_input, self.kind)
    }
}

/// Optional Source-input prefix followed by exactly one seek on an unknown
/// descriptor.
///
/// Only the authored seek coordinates survive in this record. The operation
/// tag, scoped provider, failed result, error, unknown descriptor input, and
/// empty side lanes are fixed by the record type; no provider descriptor is
/// retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorSeekReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    offset: i64,
    whence: i32,
}

impl FilesystemInputUnknownDescriptorSeekReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        offset: i64,
        whence: i32,
    ) -> Self {
        Self {
            source_input,
            offset,
            whence,
        }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn offset(&self) -> i64 {
        self.offset
    }

    pub const fn whence(&self) -> i32 {
        self.whence
    }

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, i64, i32) {
        (self.source_input, self.offset, self.whence)
    }
}

/// One write-gated scalar operation whose unknown descriptor deterministically
/// fails with `EBADF`. Each variant retains only its authored scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemInputUnknownDescriptorWriteOperationReplayKind {
    SetFilePermissions { mode: u32 },
    SetLength { length: i64 },
    LockFile { operation: i32 },
    ChangeFileOwner { uid: i32, gid: i32 },
}

impl FilesystemInputUnknownDescriptorWriteOperationReplayKind {
    const fn operation_tag(self) -> u16 {
        match self {
            Self::SetFilePermissions { .. } => 17,
            Self::SetLength { .. } => 41,
            Self::LockFile { .. } => 46,
            Self::ChangeFileOwner { .. } => 49,
        }
    }

    fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
        match self {
            Self::SetFilePermissions { mode } => vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::U32(mode),
            }],
            Self::SetLength { length } => vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I64(length),
            }],
            Self::LockFile { operation } => vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I32(operation),
            }],
            Self::ChangeFileOwner { uid, gid } => vec![
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I32(uid),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I32(gid),
                },
            ],
        }
    }
}

/// Optional exact Source-input prefix followed by one closed write-gated
/// scalar operation on an unknown descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorWriteOperationReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
}

impl FilesystemInputUnknownDescriptorWriteOperationReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    ) -> Self {
        Self { source_input, kind }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> FilesystemInputUnknownDescriptorWriteOperationReplayKind {
        self.kind
    }

    fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    ) {
        (self.source_input, self.kind)
    }
}

impl FilesystemReplay {
    /// Construct the closed optional-Source plus one unknown-descriptor
    /// operation rung from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_operation_record(
        record: FilesystemInputUnknownDescriptorOperationReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_parts();
        unknown_descriptor_failure_replay_from_record(
            source_input,
            unknown_descriptor_operation_attempt(kind),
            unknown_descriptor_operation_attempt_is_exact,
            "operation",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one operand-free unknown-descriptor operation.
    pub fn from_input_unknown_descriptor_operation_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_descriptor_failure_replay_from_observations(
            observations,
            unknown_descriptor_operation_attempt_is_exact,
            "operation",
        )
    }

    /// Construct the closed optional-Source plus unknown-descriptor seek rung
    /// from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_seek_record(
        record: FilesystemInputUnknownDescriptorSeekReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, offset, whence) = record.into_parts();
        unknown_descriptor_failure_replay_from_record(
            source_input,
            unknown_descriptor_seek_attempt(offset, whence),
            unknown_descriptor_seek_attempt_is_exact,
            "seek",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one seek on an unknown descriptor.
    pub fn from_input_unknown_descriptor_seek_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_descriptor_failure_replay_from_observations(
            observations,
            unknown_descriptor_seek_attempt_is_exact,
            "seek",
        )
    }

    /// Construct the closed optional-Source plus one write-gated scalar
    /// unknown-descriptor operation from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_write_operation_record(
        record: FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_parts();
        unknown_descriptor_failure_replay_from_record(
            source_input,
            unknown_descriptor_write_operation_attempt(kind),
            unknown_descriptor_write_operation_attempt_is_exact,
            "write operation",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one write-gated scalar operation on an unknown descriptor.
    pub fn from_input_unknown_descriptor_write_operation_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_descriptor_failure_replay_from_observations(
            observations,
            unknown_descriptor_write_operation_attempt_is_exact,
            "write operation",
        )
    }
}

fn unknown_descriptor_failure_replay_from_record(
    source_input: Option<FilesystemSourceInputReplayRecord>,
    operation: FilesystemOperationAttempt,
    operation_is_exact: fn(&FilesystemOperationAttempt) -> bool,
    operation_name: &str,
) -> Result<FilesystemReplay, String> {
    let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
    attempts.push(operation);
    validate_filesystem_replay_size(&attempts)?;
    let (operation, source_attempts) = attempts
        .split_last()
        .expect("typed unknown-descriptor failure record is nonempty");
    validate_unknown_descriptor_failure_attempts(
        source_attempts,
        operation,
        &[],
        operation_is_exact,
        operation_name,
    )?;
    Ok(FilesystemReplay {
        attempts: attempts.into(),
        expected_included_sources: std::sync::Arc::from([]),
    })
}

fn unknown_descriptor_failure_replay_from_observations(
    observations: &EvaluationObservations,
    operation_is_exact: fn(&FilesystemOperationAttempt) -> bool,
    operation_name: &str,
) -> Result<FilesystemReplay, String> {
    if observations.filesystem_operation_schema_version()
        != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
    {
        return Err("filesystem replay observation schema is not current".to_owned());
    }
    let attempts = observations.filesystem_operation_attempts();
    validate_filesystem_replay_size(attempts)?;
    let (operation, source_attempts) = attempts.split_last().ok_or_else(|| {
        format!("filesystem replay requires one failed unknown-descriptor {operation_name}")
    })?;
    validate_unknown_descriptor_failure_attempts(
        source_attempts,
        operation,
        observations.build_included_sources(),
        operation_is_exact,
        operation_name,
    )?;
    Ok(FilesystemReplay {
        attempts: attempts.to_vec().into(),
        expected_included_sources: std::sync::Arc::from([]),
    })
}

fn validate_unknown_descriptor_failure_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    operation: &FilesystemOperationAttempt,
    included_sources: &[BuildIncludedSource],
    operation_is_exact: fn(&FilesystemOperationAttempt) -> bool,
    operation_name: &str,
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(format!(
            "filesystem replay failed unknown-descriptor {operation_name} cannot hand off generated sources"
        ));
    }
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if !operation_is_exact(operation) {
        return Err(format!(
            "filesystem replay failed unknown-descriptor {operation_name} lanes are inconsistent"
        ));
    }
    Ok(())
}

pub(crate) fn unknown_descriptor_operation_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<FilesystemInputUnknownDescriptorOperationReplayKind> {
    let kind = FilesystemInputUnknownDescriptorOperationReplayKind::from_operation_tag(
        attempt.operation_tag,
    )?;
    (attempt.scalar_operands.is_empty()
        && unknown_descriptor_failure_has_exact_common_shape(attempt, kind.operation_tag()))
    .then_some(kind)
}

fn unknown_descriptor_operation_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    unknown_descriptor_operation_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_operation_attempt(
    kind: FilesystemInputUnknownDescriptorOperationReplayKind,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(kind.operation_tag(), Vec::new())
}

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

fn unknown_descriptor_seek_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
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

fn unknown_descriptor_write_operation_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_write_operation_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_write_operation_attempt(
    kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands())
}

pub(crate) fn unknown_descriptor_failure_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_operation_attempt_is_exact(attempt)
        || unknown_descriptor_seek_attempt_is_exact(attempt)
        || unknown_descriptor_write_operation_attempt_is_exact(attempt)
}

fn unknown_descriptor_failure_has_exact_common_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: observed_operation_tag,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_RESULT),
                post_error: BAD_DESCRIPTOR_ERROR,
            }),
            scalar_operands: _,
            byte_operands,
            path_like_operands,
            rooted_path_operand_resolutions,
            returned_paths,
            observed_byte_regions,
            metadata_observations,
            mutable_byte_operand_resolutions,
            mutable_i64_operand_resolutions,
            mutable_byte_operands,
            mutable_i64_operands,
            authorized_paths,
            logical_handle_inputs,
            logical_handle_output: None,
            retired_logical_handles,
            grant_refusals,
        } if *observed_operation_tag == operation_tag
            && byte_operands.is_empty()
            && path_like_operands.is_empty()
            && rooted_path_operand_resolutions.is_empty()
            && returned_paths.is_empty()
            && observed_byte_regions.is_empty()
            && metadata_observations.is_empty()
            && mutable_byte_operand_resolutions.is_empty()
            && mutable_i64_operand_resolutions.is_empty()
            && mutable_byte_operands.is_empty()
            && mutable_i64_operands.is_empty()
            && authorized_paths.is_empty()
            && matches!(
                logical_handle_inputs.as_slice(),
                [FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Unknown,
                }]
            )
            && retired_logical_handles.is_empty()
            && grant_refusals.is_empty()
    )
}

fn unknown_descriptor_failure_attempt(
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
