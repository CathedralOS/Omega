//! Validating a replay: retention weights and size limits, output extents
//! and times, expected included sources, source input attempts and exact
//! refusal attempts.

use crate::filesystem_replay::directories::source_attempts_use_root;
use crate::filesystem_replay::native_query_chains::source_native_handle_query_chain_is_exact;
use crate::filesystem_replay::output_attempts::filesystem_output_attempt_tag;
use crate::filesystem_replay::output_failures::{
    FilesystemInputOutputAbsentRemovesReplayRecord, output_absent_remove_record_from_attempt,
};
use crate::filesystem_replay::replay_records::output_file_attempt_count;
use crate::filesystem_replay::source_attempts::source_path_metadata_attempt_is_exact;
use crate::filesystem_replay::source_directories::source_directory_chain_is_exact;
use crate::filesystem_replay::source_read_links::source_read_link_attempt_is_exact;
use crate::filesystem_replay::source_write_refusals::source_write_refusal_record_from_attempt;
use crate::filesystem_replay::{
    FilesystemOutputFileOperationReplayRecord, FilesystemOutputFileReplayRecord,
    MAX_FILESYSTEM_REPLAY_RETAINED_BYTES, MAX_INCLUDED_BUILD_SOURCES,
};
use crate::filesystem_replay::{
    handle_failures, native_mutation_failures, open_at_failures, read_dir_failures,
    unlink_at_failures,
};
use crate::{BuildIncludedSource, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome};

pub(crate) fn source_write_refusal_attempt_is_exact(
    attempt: &crate::FilesystemOperationAttempt,
) -> bool {
    source_write_refusal_record_from_attempt(attempt).is_ok()
}

pub(crate) fn unknown_input_handle_failure_attempt_is_exact(
    attempt: &crate::FilesystemOperationAttempt,
) -> bool {
    handle_failures::unknown_input_handle_failure_attempt_is_exact(attempt)
        || open_at_failures::unknown_descriptor_open_at_attempt_is_exact(attempt)
        || read_dir_failures::unknown_descriptor_read_dir_attempt_is_exact(attempt)
        || unlink_at_failures::unknown_descriptor_unlink_at_attempt_is_exact(attempt)
        || native_mutation_failures::unknown_native_handle_mutation_attempt_is_exact(attempt)
}

pub(crate) fn unknown_descriptor_bad_descriptor_failure_attempt_is_exact(
    attempt: &crate::FilesystemOperationAttempt,
) -> bool {
    unknown_input_handle_failure_attempt_is_exact(attempt)
        && matches!(
            attempt,
            crate::FilesystemOperationAttempt {
                outcome: Some(crate::FilesystemOperationAttemptOutcome::Returned {
                    post_error: 9,
                    ..
                }),
                logical_handle_inputs,
                ..
            } if matches!(
                logical_handle_inputs.as_slice(),
                [crate::FilesystemLogicalHandleInput {
                    kind: crate::FilesystemLogicalHandleKind::Descriptor,
                    resolution: crate::FilesystemLogicalHandleInputResolution::Unknown,
                    ..
                }]
            )
        )
}

pub(crate) const MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT: usize = 16 * 1024 * 1024;
// Attempt = 16 fixed bytes + fifteen 8-byte lane counts + the 19-byte maximum
// logical-output row. The remaining weights are each row's fixed-width maximum
// in encode_attempt; byte-bearing lanes add their payloads below.

pub(crate) const FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT: usize = 155;

const FILESYSTEM_REPLAY_SCALAR_OPERAND_RETENTION_WEIGHT: usize = 10;

pub(crate) const FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT: usize = 9;

const FILESYSTEM_REPLAY_PATH_LIKE_OPERAND_RETENTION_WEIGHT: usize = 9;

const FILESYSTEM_REPLAY_ROOTED_PATH_RETENTION_WEIGHT: usize = 10;

const FILESYSTEM_REPLAY_RETURNED_PATH_RETENTION_WEIGHT: usize = 11;

const FILESYSTEM_REPLAY_OBSERVED_REGION_RETENTION_WEIGHT: usize = 18;

const FILESYSTEM_REPLAY_METADATA_RETENTION_WEIGHT: usize = 102;

const FILESYSTEM_REPLAY_MUTABLE_BYTE_RESOLUTION_RETENTION_WEIGHT: usize = 9;

const FILESYSTEM_REPLAY_MUTABLE_I64_RESOLUTION_RETENTION_WEIGHT: usize = 9;

const FILESYSTEM_REPLAY_MUTABLE_BYTE_RETENTION_WEIGHT: usize = 17;

const FILESYSTEM_REPLAY_MUTABLE_I64_RETENTION_WEIGHT: usize = 17;

const FILESYSTEM_REPLAY_AUTHORIZED_PATH_RETENTION_WEIGHT: usize = 11;

const FILESYSTEM_REPLAY_LOGICAL_INPUT_RETENTION_WEIGHT: usize = 11;

const FILESYSTEM_REPLAY_RETIRED_HANDLE_RETENTION_WEIGHT: usize = 8;

const FILESYSTEM_REPLAY_GRANT_REFUSAL_RETENTION_WEIGHT: usize = 3;

pub(crate) fn validate_output_time_replay_retention(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    outputs
        .iter()
        .flat_map(|output| output.operations.iter())
        .try_fold(0usize, |retained, operation| {
            let FilesystemOutputFileOperationReplayRecord::SetFileTimes { times } = operation
            else {
                return Some(retained);
            };
            retained.checked_add(times.len().checked_mul(3)?)
        })
        .filter(|retained| *retained <= MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
        .map(|_| ())
        .ok_or_else(|| {
            format!(
                "filesystem replay Output time carriers exceed the {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte retained-evidence ceiling"
            )
        })
}

pub(crate) fn validate_output_replay_extents(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    outputs
        .iter()
        .try_fold(0usize, |total, output| {
            total
                .checked_add(output.replayed_extents()?.1)
                .filter(|extent| *extent <= MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
                .ok_or_else(|| {
                    format!(
                        "filesystem replay Output exceeds its {MAX_FILESYSTEM_REPLAY_RETAINED_BYTES}-byte aggregate extent ceiling"
                    )
                })
        })
        .map(|_| ())
}

pub(crate) fn validate_expected_included_sources(
    outputs: &[FilesystemOutputFileReplayRecord],
    included_sources: &[BuildIncludedSource],
    source_attempt_count: usize,
) -> Result<(), String> {
    if included_sources.len() > MAX_INCLUDED_BUILD_SOURCES {
        return Err(format!(
            "filesystem replay exceeds its {MAX_INCLUDED_BUILD_SOURCES}-source handoff ceiling"
        ));
    }
    let total_attempt_count = outputs
        .iter()
        .try_fold(source_attempt_count, |count, output| {
            count.checked_add(output_file_attempt_count(output)?)
        })
        .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
    let mut previous_ordinal = source_attempt_count;
    for (handoff_index, included) in included_sources.iter().enumerate() {
        if included.filesystem_attempt_ordinal() < previous_ordinal {
            return Err(
                "filesystem replay included-source handoff ordinals must be nondecreasing"
                    .to_owned(),
            );
        }
        previous_ordinal = included.filesystem_attempt_ordinal();
        if included_sources[..handoff_index].iter().any(|prior| {
            prior.root() == included.root() && prior.relative_path() == included.relative_path()
        }) {
            return Err(
                "filesystem replay included-source handoff names one output more than once"
                    .to_owned(),
            );
        }
        let Some(output_index) = outputs.iter().position(|output| {
            output.output_root() == included.root()
                && output.output_relative_path() == included.relative_path()
        }) else {
            return Err(
                "filesystem replay included-source handoff has no matching output file".to_owned(),
            );
        };
        let earliest_ordinal = outputs[..=output_index]
            .iter()
            .try_fold(source_attempt_count, |count, output| {
                count.checked_add(output_file_attempt_count(output)?)
            })
            .ok_or_else(|| "filesystem replay event count overflowed".to_owned())?;
        if included.filesystem_attempt_ordinal() < earliest_ordinal
            || included.filesystem_attempt_ordinal() > total_attempt_count
        {
            return Err(
                "filesystem replay included-source handoff must follow its exact Output close"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_source_input_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<(), String> {
    let membership = super::source_stream::SourceEventMembership::discover(attempts)?;
    if membership.source_attempts.is_empty()
        || membership.source_attempts.iter().any(|source| !source)
    {
        return Err("filesystem replay requires only complete Source events".to_owned());
    }
    for event in membership.project(attempts) {
        validate_source_event_attempts(&event)?;
    }
    Ok(())
}

pub(crate) fn validate_source_event_attempts(
    attempts: &[&FilesystemOperationAttempt],
) -> Result<(), String> {
    let mut cursor = 0;
    let mut event_count = 0;
    while cursor < attempts.len() {
        if filesystem_output_attempt_tag(attempts[cursor].operation_tag()) {
            break;
        }
        if attempts[cursor].operation_tag() == 21 {
            if !source_read_link_attempt_is_exact(attempts[cursor]) {
                return Err(
                    "bounded filesystem replay source read-link event is inconsistent".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(attempts[cursor].operation_tag(), 38 | 40) {
            if !source_path_metadata_attempt_is_exact(attempts[cursor]) {
                return Err("bounded filesystem replay source metadata is inconsistent".to_owned());
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if attempts[cursor].operation_tag() == 28 {
            let event_start = cursor;
            cursor += 1;
            while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 31 | 35) {
                cursor += 1;
            }
            if cursor == attempts.len()
                || attempts[cursor].operation_tag() != 29
                || !source_native_handle_query_chain_is_exact(&attempts[event_start..=cursor])
            {
                return Err(
                    "bounded filesystem replay Source native-handle query chain is inconsistent"
                        .to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if attempts[cursor].operation_tag() != 2 {
            return Err(
                "bounded filesystem replay requires ordered source-input events".to_owned(),
            );
        }
        let event_start = cursor;
        cursor += 1;
        if cursor < attempts.len() && attempts[cursor].operation_tag() == 39 {
            cursor += 1;
            if cursor == attempts.len() || attempts[cursor].operation_tag() != 8 {
                return Err(
                    "bounded filesystem replay requires ordered source-input events".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
            while cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
                cursor += 1;
            }
            if cursor == attempts.len()
                || attempts[cursor].operation_tag() != 8
                || !source_directory_chain_is_exact(&attempts[event_start..=cursor])
            {
                return Err(
                    "bounded filesystem replay Source directory chain is inconsistent".to_owned(),
                );
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        let reads_start = cursor;
        while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 4 | 6) {
            cursor += 1;
        }
        if cursor == reads_start
            || cursor == attempts.len()
            || attempts[cursor].operation_tag() != 8
        {
            return Err(
                "bounded filesystem replay requires ordered source-input events".to_owned(),
            );
        }
        cursor += 1;
        event_count += 1;
    }
    if event_count == 0 || cursor != attempts.len() {
        return Err("bounded filesystem replay requires source-input events".to_owned());
    }
    for (index, attempt) in attempts.iter().enumerate() {
        if !matches!(
            attempt.outcome,
            Some(FilesystemOperationAttemptOutcome::Returned { .. })
        ) {
            return Err(format!(
                "filesystem replay event {index} did not return normally"
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_filesystem_replay_size(
    attempts: &[FilesystemOperationAttempt],
) -> Result<(), String> {
    let mut retained = attempts
        .len()
        .checked_mul(FILESYSTEM_REPLAY_ATTEMPT_RETENTION_WEIGHT);
    let mut add = |weight: usize| {
        retained = retained
            .and_then(|total| total.checked_add(weight))
            .filter(|total| *total <= MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT);
    };
    let lane_weight = |length: usize, weight: usize| length.saturating_mul(weight);
    for attempt in attempts {
        add(lane_weight(
            attempt.scalar_operands.len(),
            FILESYSTEM_REPLAY_SCALAR_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.byte_operands.len(),
            FILESYSTEM_REPLAY_BYTE_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.path_like_operands.len(),
            FILESYSTEM_REPLAY_PATH_LIKE_OPERAND_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.rooted_path_operand_resolutions.len(),
            FILESYSTEM_REPLAY_ROOTED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.returned_paths.len(),
            FILESYSTEM_REPLAY_RETURNED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.observed_byte_regions.len(),
            FILESYSTEM_REPLAY_OBSERVED_REGION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.metadata_observations.len(),
            FILESYSTEM_REPLAY_METADATA_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_byte_operand_resolutions.len(),
            FILESYSTEM_REPLAY_MUTABLE_BYTE_RESOLUTION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_i64_operand_resolutions.len(),
            FILESYSTEM_REPLAY_MUTABLE_I64_RESOLUTION_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_byte_operands.len(),
            FILESYSTEM_REPLAY_MUTABLE_BYTE_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.mutable_i64_operands.len(),
            FILESYSTEM_REPLAY_MUTABLE_I64_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.authorized_paths.len(),
            FILESYSTEM_REPLAY_AUTHORIZED_PATH_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.logical_handle_inputs.len(),
            FILESYSTEM_REPLAY_LOGICAL_INPUT_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.retired_logical_handles.len(),
            FILESYSTEM_REPLAY_RETIRED_HANDLE_RETENTION_WEIGHT,
        ));
        add(lane_weight(
            attempt.grant_refusals.len(),
            FILESYSTEM_REPLAY_GRANT_REFUSAL_RETENTION_WEIGHT,
        ));
        for operand in &attempt.byte_operands {
            add(operand.bytes.len());
        }
        for operand in &attempt.path_like_operands {
            add(operand.bytes.len());
        }
        for operand in &attempt.rooted_path_operand_resolutions {
            add(operand.relative_path.len());
        }
        for returned in &attempt.returned_paths {
            add(returned.bytes.len());
        }
        for operand in &attempt.mutable_byte_operand_resolutions {
            add(operand.bytes.len());
        }
        for operand in &attempt.mutable_byte_operands {
            add(operand.pre_bytes.len());
            add(operand.post_bytes.len());
        }
        for path in &attempt.authorized_paths {
            add(path.relative_path.len());
        }
    }
    retained
        .map(|_| ())
        .ok_or_else(|| format!(
            "filesystem replay attempts exceed their {MAX_FILESYSTEM_REPLAY_RETENTION_WEIGHT}-unit deterministic retention-weight ceiling"
        ))
}

pub(crate) fn output_absent_remove_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    output_absent_remove_record_from_attempt(attempt).is_ok()
}

pub(crate) fn validate_output_absent_remove_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    attempts: &[FilesystemOperationAttempt],
    included_sources: &[BuildIncludedSource],
) -> Result<(), String> {
    if attempts.is_empty() {
        return Err("filesystem replay requires at least one absent Output remove".to_owned());
    }
    if !included_sources.is_empty() {
        return Err(
            "filesystem replay failure-only Output operations cannot hand off generated sources"
                .to_owned(),
        );
    }
    let mut records = Vec::new();
    records
        .try_reserve_exact(attempts.len())
        .map_err(|_| "filesystem replay absent Output remove allocation failed".to_owned())?;
    for attempt in attempts {
        records.push(output_absent_remove_record_from_attempt(attempt)?);
    }
    let record = FilesystemInputOutputAbsentRemovesReplayRecord::new(None, records)?;
    let (_, records) = record.into_parts();
    let output_root = records[0].output_root();
    if source_attempts_use_root(source_attempts, output_root) {
        return Err("filesystem replay Source and Output roots must be distinct".to_owned());
    }
    Ok(())
}
