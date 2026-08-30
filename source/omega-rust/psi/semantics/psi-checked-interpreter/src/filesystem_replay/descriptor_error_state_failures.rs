//! Ordered replay for a modeled bad-descriptor failure and `errno` read.

use super::handle_failures::{
    FilesystemInputUnknownDescriptorOperationReplayRecord, unknown_descriptor_operation_attempt,
    unknown_descriptor_operation_from_exact_attempt,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemReplay, source_input_record_attempts,
    validate_filesystem_replay_size, validate_source_input_attempts,
};

const ERRNO_OPERATION_TAG: u16 = 50;
const BAD_DESCRIPTOR_ERROR: i32 = 9;

/// Optional exact Source input followed by one operand-free unknown-descriptor
/// failure and its immediate `errno` observation.
///
/// The error-state row is admitted only as part of this exact ordered pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord {
    operation: FilesystemInputUnknownDescriptorOperationReplayRecord,
}

impl FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord {
    pub const fn new(operation: FilesystemInputUnknownDescriptorOperationReplayRecord) -> Self {
        Self { operation }
    }

    pub const fn operation(&self) -> &FilesystemInputUnknownDescriptorOperationReplayRecord {
        &self.operation
    }

    fn into_operation(self) -> FilesystemInputUnknownDescriptorOperationReplayRecord {
        self.operation
    }
}

impl FilesystemReplay {
    /// Construct one exact operand-free `EBADF` failure and immediate `errno`.
    pub fn from_input_unknown_descriptor_operation_with_errno_record(
        record: FilesystemInputUnknownDescriptorOperationWithErrnoReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_operation().into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        attempts.push(unknown_descriptor_operation_attempt(kind));
        attempts.push(errno_after_bad_descriptor_attempt());
        validate_filesystem_replay_size(&attempts)?;
        validate_descriptor_operation_with_errno_attempts(&attempts, &[])?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate exactly one operand-free unknown-descriptor failure and its
    /// immediate error-state read after an optional exact Source prefix.
    pub fn from_input_unknown_descriptor_operation_with_errno_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        validate_descriptor_operation_with_errno_attempts(
            attempts,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}

fn validate_descriptor_operation_with_errno_attempts(
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failed descriptor operation and errno read cannot hand off generated sources"
                .to_owned(),
        );
    }
    let suffix_start = attempts.len().checked_sub(2).ok_or_else(|| {
        "filesystem replay requires one descriptor operation followed by errno".to_owned()
    })?;
    let source_attempts = &attempts[..suffix_start];
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if unknown_descriptor_operation_from_exact_attempt(&attempts[suffix_start]).is_none()
        || !errno_after_bad_descriptor_attempt_is_exact(&attempts[suffix_start + 1])
    {
        return Err(
            "filesystem replay descriptor operation and errno lanes are inconsistent".to_owned(),
        );
    }
    Ok(())
}

pub(super) fn errno_after_bad_descriptor_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: ERRNO_OPERATION_TAG,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(error),
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
        } if *error == i64::from(BAD_DESCRIPTOR_ERROR)
            && scalar_operands.is_empty()
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
            && logical_handle_inputs.is_empty()
            && retired_logical_handles.is_empty()
            && grant_refusals.is_empty()
    )
}

pub(crate) fn ordered_descriptor_error_state_attempt_is_replayed(
    attempts: &[FilesystemOperationAttempt],
    attempt_index: usize,
) -> bool {
    attempt_index.checked_sub(1).is_some_and(|operation_index| {
        attempts
            .get(operation_index)
            .and_then(unknown_descriptor_operation_from_exact_attempt)
            .is_some()
            && attempts
                .get(attempt_index)
                .is_some_and(errno_after_bad_descriptor_attempt_is_exact)
    })
}

pub(super) fn errno_after_bad_descriptor_attempt() -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: ERRNO_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(i64::from(BAD_DESCRIPTOR_ERROR)),
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
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}
