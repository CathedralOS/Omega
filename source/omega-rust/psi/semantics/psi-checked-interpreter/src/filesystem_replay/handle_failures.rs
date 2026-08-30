use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, FilesystemReplay,
    FilesystemSourceInputReplayRecord, source_input_record_attempts,
    validate_filesystem_replay_size, validate_source_input_attempts,
};

const CLOSE_OPERATION_TAG: u16 = 8;
const UNKNOWN_DESCRIPTOR_RESULT: i64 = -1;
const BAD_DESCRIPTOR_ERROR: i32 = 9;

/// Optional Source-input prefix followed by exactly one failed close of an
/// unknown descriptor.
///
/// The close contributes no authored coordinates to this record: its provider,
/// result, error, logical input, and empty side lanes are fixed by the record
/// type. In particular, the raw provider descriptor is not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorCloseReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
}

impl FilesystemInputUnknownDescriptorCloseReplayRecord {
    pub fn new(source_input: Option<FilesystemSourceInputReplayRecord>) -> Self {
        Self { source_input }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub(crate) fn into_source_input(self) -> Option<FilesystemSourceInputReplayRecord> {
        self.source_input
    }
}

impl FilesystemReplay {
    /// Construct the closed optional-Source plus one failed unknown-descriptor
    /// close rung from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_close_record(
        record: FilesystemInputUnknownDescriptorCloseReplayRecord,
    ) -> Result<Self, String> {
        let mut attempts = record
            .into_source_input()
            .map_or_else(Vec::new, source_input_record_attempts);
        attempts.push(unknown_descriptor_close_attempt());
        validate_filesystem_replay_size(&attempts)?;
        let (close, source_attempts) = attempts
            .split_last()
            .expect("typed unknown-descriptor close record is nonempty");
        validate_input_unknown_descriptor_close_attempts(source_attempts, close, &[])?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one failed close of an unknown descriptor.
    pub fn from_input_unknown_descriptor_close_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        let (close, source_attempts) = attempts.split_last().ok_or_else(|| {
            "filesystem replay requires one failed unknown-descriptor close".to_owned()
        })?;
        validate_input_unknown_descriptor_close_attempts(
            source_attempts,
            close,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}

fn validate_input_unknown_descriptor_close_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    close: &FilesystemOperationAttempt,
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failed unknown-descriptor close cannot hand off generated sources"
                .to_owned(),
        );
    }
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if !unknown_descriptor_close_attempt_is_exact(close) {
        return Err(
            "filesystem replay failed unknown-descriptor close lanes are inconsistent".to_owned(),
        );
    }
    Ok(())
}

pub(crate) fn unknown_descriptor_close_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: CLOSE_OPERATION_TAG,
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
}

pub(crate) fn unknown_descriptor_close_attempt() -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: CLOSE_OPERATION_TAG,
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
