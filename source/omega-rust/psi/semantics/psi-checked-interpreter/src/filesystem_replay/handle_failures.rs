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

impl FilesystemReplay {
    /// Construct the closed optional-Source plus one unknown-descriptor
    /// operation rung from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_operation_record(
        record: FilesystemInputUnknownDescriptorOperationReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        attempts.push(unknown_descriptor_operation_attempt(kind));
        validate_filesystem_replay_size(&attempts)?;
        let (operation, source_attempts) = attempts
            .split_last()
            .expect("typed unknown-descriptor operation record is nonempty");
        validate_input_unknown_descriptor_operation_attempts(source_attempts, operation, &[])?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one operand-free unknown-descriptor operation.
    pub fn from_input_unknown_descriptor_operation_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        let (operation, source_attempts) = attempts.split_last().ok_or_else(|| {
            "filesystem replay requires one failed unknown-descriptor operation".to_owned()
        })?;
        validate_input_unknown_descriptor_operation_attempts(
            source_attempts,
            operation,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Construct the closed optional-Source plus unknown-descriptor seek rung
    /// from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_seek_record(
        record: FilesystemInputUnknownDescriptorSeekReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, offset, whence) = record.into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        attempts.push(unknown_descriptor_seek_attempt(offset, whence));
        validate_filesystem_replay_size(&attempts)?;
        let (seek, source_attempts) = attempts
            .split_last()
            .expect("typed unknown-descriptor seek record is nonempty");
        validate_input_unknown_descriptor_seek_attempts(source_attempts, seek, &[])?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one seek on an unknown descriptor.
    pub fn from_input_unknown_descriptor_seek_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        let (seek, source_attempts) = attempts.split_last().ok_or_else(|| {
            "filesystem replay requires one failed unknown-descriptor seek".to_owned()
        })?;
        validate_input_unknown_descriptor_seek_attempts(
            source_attempts,
            seek,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}

fn validate_input_unknown_descriptor_operation_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    operation: &FilesystemOperationAttempt,
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failed unknown-descriptor operation cannot hand off generated sources"
                .to_owned(),
        );
    }
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if unknown_descriptor_operation_from_exact_attempt(operation).is_none() {
        return Err(
            "filesystem replay failed unknown-descriptor operation lanes are inconsistent"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_input_unknown_descriptor_seek_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    seek: &FilesystemOperationAttempt,
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failed unknown-descriptor seek cannot hand off generated sources"
                .to_owned(),
        );
    }
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if unknown_descriptor_seek_from_exact_attempt(seek).is_none() {
        return Err(
            "filesystem replay failed unknown-descriptor seek lanes are inconsistent".to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn unknown_descriptor_operation_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<FilesystemInputUnknownDescriptorOperationReplayKind> {
    let kind = FilesystemInputUnknownDescriptorOperationReplayKind::from_operation_tag(
        attempt.operation_tag,
    )?;
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: _,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_RESULT),
                post_error: BAD_DESCRIPTOR_ERROR,
            }),
            scalar_operands,
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
        } if scalar_operands.is_empty()
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
    .then_some(kind)
}

pub(crate) fn unknown_descriptor_operation_attempt(
    kind: FilesystemInputUnknownDescriptorOperationReplayKind,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: kind.operation_tag(),
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_RESULT),
            post_error: BAD_DESCRIPTOR_ERROR,
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
            resolution: FilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
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
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: SEEK_OPERATION_TAG,
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
        } if byte_operands.is_empty()
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
    .then_some((*offset, *whence))
}

pub(crate) fn unknown_descriptor_seek_attempt(
    offset: i64,
    whence: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: SEEK_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_RESULT),
            post_error: BAD_DESCRIPTOR_ERROR,
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
            resolution: FilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}
