//! Replaying recorded unknown handle failures from records and
//! observations.

use crate::filesystem_replay::handle_failures::FilesystemInputUnknownDescriptorOperationReplayKind;
use crate::filesystem_replay::handle_failures::descriptor_attempts::{
    unknown_descriptor_failure_attempt, unknown_descriptor_failure_has_exact_common_shape,
};
use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
    FilesystemOperationAttempt, FilesystemReplay, FilesystemSourceInputReplayRecord,
    source_input_record_attempts, validate_filesystem_replay_size, validate_source_input_attempts,
};

pub(crate) fn unknown_handle_input_failure_replay_from_record(
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
    FilesystemReplay::from_validated(attempts.into(), std::sync::Arc::from([]))
}

pub(crate) fn unknown_handle_input_failure_replay_from_observations(
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
    FilesystemReplay::from_validated(attempts.to_vec().into(), std::sync::Arc::from([]))
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

pub(crate) fn unknown_descriptor_operation_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_operation_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_operation_attempt(
    kind: FilesystemInputUnknownDescriptorOperationReplayKind,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(kind.operation_tag(), Vec::new())
}
