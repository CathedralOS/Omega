//! Ordered replay for a modeled native-handle failure and its error-state read.

use super::handle_failures::{
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
};
use super::native_mutation_failures::{
    FilesystemInputUnknownNativeHandleMutationReplayRecord, unknown_native_handle_mutation_attempt,
    unknown_native_handle_mutation_attempt_is_exact,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemReplay, source_input_record_attempts,
    validate_filesystem_replay_size, validate_source_input_attempts,
};

const GET_LAST_ERROR_OPERATION_TAG: u16 = 35;
const INVALID_HANDLE_ERROR: i32 = 6;

/// Optional exact Source input followed by one modeled invalid-native-handle
/// mutation and its immediate `get_last_error` observation.
///
/// The second row is meaningful only as part of this exact ordered pair. This
/// record does not admit a standalone read of provider error state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord {
    mutation: FilesystemInputUnknownNativeHandleMutationReplayRecord,
}

impl FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord {
    pub const fn new(mutation: FilesystemInputUnknownNativeHandleMutationReplayRecord) -> Self {
        Self { mutation }
    }

    pub const fn mutation(&self) -> &FilesystemInputUnknownNativeHandleMutationReplayRecord {
        &self.mutation
    }

    fn into_mutation(self) -> FilesystemInputUnknownNativeHandleMutationReplayRecord {
        self.mutation
    }
}

impl FilesystemReplay {
    /// Construct the exact ordered invalid-handle mutation and last-error pair.
    pub fn from_input_unknown_native_handle_mutation_with_last_error_record(
        record: FilesystemInputUnknownNativeHandleMutationWithLastErrorReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_mutation().into_parts();
        let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
        attempts.push(unknown_native_handle_mutation_attempt(kind));
        attempts.push(get_last_error_after_invalid_handle_attempt());
        validate_filesystem_replay_size(&attempts)?;
        validate_native_mutation_with_last_error_attempts(&attempts, &[])?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate observations containing exactly the ordered invalid-handle
    /// mutation and immediate last-error read after an optional Source prefix.
    pub fn from_input_unknown_native_handle_mutation_with_last_error_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        validate_native_mutation_with_last_error_attempts(
            attempts,
            observations.build_included_sources(),
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Append immediate `get_last_error` to any already exact unknown-native-
    /// handle replay whose modeled failure establishes invalid-handle error 6.
    pub fn with_immediate_last_error_after_unknown_native_handle_failure(
        self,
    ) -> Result<Self, String> {
        if !self.expected_included_sources.is_empty() {
            return Err(
                "filesystem replay failed native-handle operation and last-error read cannot hand off generated sources"
                    .to_owned(),
            );
        }
        let mut attempts = self.attempts.to_vec();
        let (failure, source_attempts) = attempts.split_last().ok_or_else(|| {
            "filesystem replay requires one invalid-handle failure before get_last_error".to_owned()
        })?;
        if !source_attempts.is_empty() {
            validate_source_input_attempts(source_attempts)?;
        }
        if !unknown_native_handle_invalid_handle_failure_attempt_is_exact(failure) {
            return Err(
                "filesystem replay get_last_error requires an exact unknown-native-handle failure"
                    .to_owned(),
            );
        }
        attempts.push(get_last_error_after_invalid_handle_attempt());
        validate_filesystem_replay_size(&attempts)?;
        validate_native_failure_with_last_error_attempts(
            &attempts,
            &[],
            unknown_native_handle_invalid_handle_failure_attempt_is_exact,
        )?;
        Ok(Self {
            attempts: attempts.into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }

    /// Validate any exact unknown-native-handle failure followed immediately
    /// by `get_last_error`, after an optional exact Source prefix.
    pub fn from_input_unknown_native_handle_failure_with_last_error_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        if observations.filesystem_operation_schema_version()
            != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
        {
            return Err("filesystem replay observation schema is not current".to_owned());
        }
        let attempts = observations.filesystem_operation_attempts();
        validate_filesystem_replay_size(attempts)?;
        validate_native_failure_with_last_error_attempts(
            attempts,
            observations.build_included_sources(),
            unknown_native_handle_invalid_handle_failure_attempt_is_exact,
        )?;
        Ok(Self {
            attempts: attempts.to_vec().into(),
            expected_included_sources: std::sync::Arc::from([]),
        })
    }
}

fn validate_native_mutation_with_last_error_attempts(
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    validate_native_failure_with_last_error_attempts(
        attempts,
        included_sources,
        unknown_native_handle_mutation_attempt_is_exact,
    )
}

pub(super) fn get_last_error_after_invalid_handle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: GET_LAST_ERROR_OPERATION_TAG,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(error),
                post_error: INVALID_HANDLE_ERROR,
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
        } if *error == i64::from(INVALID_HANDLE_ERROR)
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

pub(crate) fn ordered_native_error_state_attempt_is_replayed(
    attempts: &[FilesystemOperationAttempt],
    attempt_index: usize,
) -> bool {
    attempt_index.checked_sub(1).is_some_and(|mutation_index| {
        attempts
            .get(mutation_index)
            .is_some_and(unknown_native_handle_invalid_handle_failure_attempt_is_exact)
            && attempts
                .get(attempt_index)
                .is_some_and(get_last_error_after_invalid_handle_attempt_is_exact)
    })
}

fn unknown_native_handle_invalid_handle_failure_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_native_handle_close_handle_attempt_is_exact(attempt)
        || unknown_native_handle_final_path_name_by_handle_attempt_is_exact(attempt)
        || unknown_native_handle_mutation_attempt_is_exact(attempt)
}

fn validate_native_failure_with_last_error_attempts(
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
    failure_is_exact: fn(&FilesystemOperationAttempt) -> bool,
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failed native-handle operation and last-error read cannot hand off generated sources"
                .to_owned(),
        );
    }
    let suffix_start = attempts.len().checked_sub(2).ok_or_else(|| {
        "filesystem replay requires one native-handle failure followed by get_last_error".to_owned()
    })?;
    let source_attempts = &attempts[..suffix_start];
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if !failure_is_exact(&attempts[suffix_start])
        || !get_last_error_after_invalid_handle_attempt_is_exact(&attempts[suffix_start + 1])
    {
        return Err(
            "filesystem replay native-handle failure and last-error lanes are inconsistent"
                .to_owned(),
        );
    }
    Ok(())
}

pub(super) fn get_last_error_after_invalid_handle_attempt() -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: GET_LAST_ERROR_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(i64::from(INVALID_HANDLE_ERROR)),
            post_error: INVALID_HANDLE_ERROR,
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
