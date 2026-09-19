//! Recognizing the replayable shape of a measured run's filesystem
//! observations and rebuilding the Output tree its receipts describe.

use crate::evidence::replay_eligibility::{
    ReceiptedOutputEntry, ReceiptedOutputFile, complete_no_output_failure_suffix_is_recognized,
    errno_tag, exact_source_write_refusal, get_last_error_tag, is_source_input_replay_record,
    operand_free_unknown_descriptor_operation_tag, source_events_are_exact,
    source_input_replay_prefix_end, unknown_descriptor_bad_descriptor_failure_tag,
    unknown_descriptor_get_osfhandle_tag, unknown_descriptor_open_at_tag,
    unknown_descriptor_read_dir_tag, unknown_descriptor_read_file_metadata_tag,
    unknown_descriptor_read_operation_tag, unknown_descriptor_set_file_times_tag,
    unknown_descriptor_unlink_at_tag, unknown_descriptor_write_operation_tag,
    unknown_descriptor_write_payload_operation_tag, unknown_native_handle_close_tag,
    unknown_native_handle_final_path_tag, unknown_native_handle_mutation_tag,
};
use build_output::{BuildStagedOutputTree, ReplayedBuildOutputEntry, empty, replayed_output_tree};
use checked_interpreter::{EvaluationObservations, FilesystemReplay};
use diagnostics::Diagnostic;

/// Validate ordinary Source/Output composition by descriptor lifetime. Exact
/// failure-only sequences remain separately selected below; a failed normal
/// validation never falls back to a less restrictive record family.
pub(super) fn recognize_filesystem_replay(
    observations: &EvaluationObservations,
) -> Option<FilesystemReplay> {
    let attempts = observations.filesystem_operation_attempts();
    // Successful Output streams use lifetime membership, never a Source prefix.
    // Select by the observed namespace operation, not by trying alternative
    // validators after a failure. Exact failure-only routes remain below.
    if attempts.iter().any(|attempt| {
        matches!(attempt.operation_tag(), 1 | 11 | 19 | 20 | 27)
            && !exact_source_write_refusal(attempt)
    }) {
        let replay = FilesystemReplay::from_input_output_observations(observations).ok()?;
        return source_events_are_exact(&replay).then_some(replay);
    }
    if let Some(operation_suffix_start) =
        source_input_replay_prefix_end(attempts).filter(|end| *end < attempts.len())
    {
        if operation_suffix_start == 0
            && attempts.len() == 1
            && exact_source_write_refusal(&attempts[0])
        {
            checked_interpreter::FilesystemReplay::from_source_write_refusal_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 2
            && unknown_descriptor_bad_descriptor_failure_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
            && errno_tag(attempts[operation_suffix_start + 1].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_failure_with_errno_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && operand_free_unknown_descriptor_operation_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_operation_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && attempts[operation_suffix_start].operation_tag() == 10
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_seek_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_open_at_tag(attempts[operation_suffix_start].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_open_at_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_unlink_at_tag(attempts[operation_suffix_start].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_unlink_at_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_read_dir_tag(attempts[operation_suffix_start].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_dir_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_write_operation_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_operation_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_set_file_times_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_set_file_times_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_read_operation_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_write_payload_operation_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_read_file_metadata_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_descriptor_get_osfhandle_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_native_handle_close_tag(attempts[operation_suffix_start].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_close_handle_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_native_handle_final_path_tag(
                attempts[operation_suffix_start].operation_tag(),
            )
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 1
            && unknown_native_handle_mutation_tag(attempts[operation_suffix_start].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_mutation_observations(
                observations,
            )
            .ok()
        } else if attempts.len() - operation_suffix_start == 2
            && (unknown_native_handle_close_tag(attempts[operation_suffix_start].operation_tag())
                || unknown_native_handle_final_path_tag(
                    attempts[operation_suffix_start].operation_tag(),
                )
                || unknown_native_handle_mutation_tag(
                    attempts[operation_suffix_start].operation_tag(),
                ))
            && get_last_error_tag(attempts[operation_suffix_start + 1].operation_tag())
        {
            checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_failure_with_last_error_observations(
                observations,
            )
            .ok()
        } else {
            checked_interpreter::FilesystemReplay::from_input_output_observations(observations).ok()
        }
    } else {
        is_source_input_replay_record(observations)
            .then(|| {
                checked_interpreter::FilesystemReplay::from_source_input_observations(observations)
            })
            .and_then(Result::ok)
    }
}

/// Whether the recognized replay ends in a complete no-output failure, which
/// stands in for staged-output custody that never came to exist.
pub(super) fn includes_complete_no_output_failure(observations: &EvaluationObservations) -> bool {
    source_input_replay_prefix_end(observations.filesystem_operation_attempts()).is_some_and(
        |suffix_start| {
            complete_no_output_failure_suffix_is_recognized(
                observations.filesystem_operation_attempts(),
                suffix_start,
            )
        },
    )
}

/// Rebuild the staged Output tree the replay's receipts describe, normalizing
/// hard links into the regular files they alias.
pub(super) fn replayed_output_tree_from_receipts(
    replay_has_no_output_attempts: bool,
    receipted_output_entries: &Option<Vec<ReceiptedOutputEntry>>,
) -> Result<Option<BuildStagedOutputTree>, Vec<Diagnostic>> {
    Ok(if replay_has_no_output_attempts {
        Some(empty())
    } else {
        receipted_output_entries
            .as_ref()
            .map(|entries| {
                let mut regular_files = std::collections::BTreeMap::new();
                let mut normalized_entries = Vec::with_capacity(entries.len());
                for entry in entries {
                    let normalized = match entry {
                        ReceiptedOutputEntry::Directory { .. }
                        | ReceiptedOutputEntry::Symlink { .. } => entry.clone(),
                        ReceiptedOutputEntry::File(file) => {
                            regular_files.insert(
                                file.relative_path.clone(),
                                (file.bytes.clone(), file.executable),
                            );
                            entry.clone()
                        }
                        ReceiptedOutputEntry::HardLink {
                            existing_relative_path,
                            relative_path,
                        } => {
                            let (bytes, executable) = regular_files
                                .get(existing_relative_path)
                                .expect("validated hard link follows a regular-file name")
                                .clone();
                            regular_files
                                .insert(relative_path.clone(), (bytes.clone(), executable));
                            ReceiptedOutputEntry::File(ReceiptedOutputFile {
                                relative_path: relative_path.clone(),
                                bytes,
                                executable,
                            })
                        }
                    };
                    normalized_entries.push(normalized);
                }
                let replayed_entries = normalized_entries
                    .iter()
                    .map(|entry| match entry {
                        ReceiptedOutputEntry::Directory { relative_path } => {
                            ReplayedBuildOutputEntry::directory(relative_path)
                        }
                        ReceiptedOutputEntry::File(file) => ReplayedBuildOutputEntry::regular_file(
                            &file.relative_path,
                            &file.bytes,
                            file.executable,
                        ),
                        ReceiptedOutputEntry::Symlink {
                            relative_path,
                            target_spelling,
                        } => {
                            ReplayedBuildOutputEntry::symbolic_link(relative_path, target_spelling)
                        }
                        ReceiptedOutputEntry::HardLink { .. } => {
                            unreachable!("hard links are normalized before staged commitment")
                        }
                    })
                    .collect::<Vec<_>>();
                replayed_output_tree(&replayed_entries)
            })
            .transpose()?
    })
}
