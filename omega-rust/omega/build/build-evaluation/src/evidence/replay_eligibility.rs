//! Supported filesystem replay sequences and retained Output reconstruction.

use crate::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY};

pub(crate) fn is_source_input_replay_record(
    observations: &checked_interpreter::EvaluationObservations,
) -> bool {
    checked_interpreter::FilesystemReplay::from_source_input_observations(observations)
        .is_ok_and(|replay| source_events_are_exact(&replay))
}

/// Projected validation keeps the exact Source-root and operand checks while
/// permitting other lifetimes to run between operations. Only the replay's
/// full chronological stream controls execution and shared error state.
pub(crate) fn source_events_are_exact(replay: &checked_interpreter::FilesystemReplay) -> bool {
    replay
        .source_input_event_attempts()
        .iter()
        .all(|event| source_input_replay_prefix_refs(event) == Some(event.len()))
}

pub(crate) const fn operand_free_unknown_descriptor_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 8 | 43 | 44 | 45)
}

pub(crate) const fn unknown_descriptor_write_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 17 | 41 | 46 | 49)
}

pub(crate) const fn unknown_descriptor_set_file_times_tag(operation_tag: u16) -> bool {
    operation_tag == 42
}

pub(crate) const fn unknown_descriptor_read_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 4 | 6)
}

pub(crate) const fn unknown_descriptor_open_at_tag(operation_tag: u16) -> bool {
    operation_tag == 14
}

pub(crate) const fn unknown_descriptor_unlink_at_tag(operation_tag: u16) -> bool {
    operation_tag == 15
}

pub(crate) const fn unknown_descriptor_read_dir_tag(operation_tag: u16) -> bool {
    operation_tag == 23
}

pub(crate) const fn unknown_descriptor_write_payload_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 5 | 7)
}

pub(crate) const fn unknown_descriptor_read_file_metadata_tag(operation_tag: u16) -> bool {
    operation_tag == 39
}

pub(crate) const fn unknown_descriptor_get_osfhandle_tag(operation_tag: u16) -> bool {
    operation_tag == 30
}

pub(crate) const fn unknown_native_handle_close_tag(operation_tag: u16) -> bool {
    operation_tag == 29
}

pub(crate) const fn unknown_native_handle_final_path_tag(operation_tag: u16) -> bool {
    operation_tag == 31
}

pub(crate) const fn unknown_native_handle_mutation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 32..=34)
}

pub(crate) const fn get_last_error_tag(operation_tag: u16) -> bool {
    operation_tag == 35
}

pub(crate) const fn errno_tag(operation_tag: u16) -> bool {
    operation_tag == 50
}

pub(crate) const fn unknown_descriptor_bad_descriptor_failure_tag(operation_tag: u16) -> bool {
    operand_free_unknown_descriptor_operation_tag(operation_tag)
        || operation_tag == 10
        || unknown_descriptor_open_at_tag(operation_tag)
        || unknown_descriptor_unlink_at_tag(operation_tag)
        || unknown_descriptor_read_dir_tag(operation_tag)
        || unknown_descriptor_write_operation_tag(operation_tag)
        || unknown_descriptor_set_file_times_tag(operation_tag)
        || unknown_descriptor_read_operation_tag(operation_tag)
        || unknown_descriptor_write_payload_operation_tag(operation_tag)
        || unknown_descriptor_read_file_metadata_tag(operation_tag)
}

pub(crate) fn exact_source_write_refusal(
    attempt: &checked_interpreter::FilesystemOperationAttempt,
) -> bool {
    let operation_is_exact = match attempt.operation_tag() {
        1 => {
            let [mode] = attempt.scalar_operands() else {
                return false;
            };
            mode.operand_ordinal() == 1
                && mode.value()
                    == checked_interpreter::FilesystemScalarOperandValue::I32(
                        checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE,
                    )
        }
        9 => attempt.scalar_operands().is_empty(),
        _ => false,
    };
    let [rooted] = attempt.rooted_path_operand_resolutions() else {
        return false;
    };
    let [refusal] = attempt.grant_refusals() else {
        return false;
    };
    operation_is_exact
        && attempt.provider() == checked_interpreter::FilesystemObservationProvider::RealScoped
        && attempt.result() == Some(checked_interpreter::FilesystemOperationResult::Scalar(-1))
        && attempt.post_error() == Some(13)
        && rooted.operand_ordinal() == 0
        && rooted.root() == BUILD_SOURCE_ROOT_IDENTITY
        && refusal.operand_ordinal() == 0
        && refusal.access() == checked_interpreter::FilesystemGrantAccess::Write
        && refusal.reason()
            == checked_interpreter::FilesystemGrantRefusalReason::OutsideGrantedRoots
        && attempt.byte_operands().is_empty()
        && attempt.path_like_operands().is_empty()
        && attempt.returned_paths().is_empty()
        && attempt.observed_byte_regions().is_empty()
        && attempt.metadata_observations().is_empty()
        && attempt.mutable_byte_operand_resolutions().is_empty()
        && attempt.mutable_i64_operand_resolutions().is_empty()
        && attempt.mutable_byte_operands().is_empty()
        && attempt.mutable_i64_operands().is_empty()
        && attempt.authorized_paths().is_empty()
        && attempt.logical_handle_inputs().is_empty()
        && attempt.logical_handle_output().is_none()
        && attempt.retired_logical_handles().is_empty()
}

pub(crate) fn complete_no_output_failure_suffix_is_recognized(
    attempts: &[checked_interpreter::FilesystemOperationAttempt],
    suffix_start: usize,
) -> bool {
    match &attempts[suffix_start..] {
        [failure] => {
            exact_source_write_refusal(failure)
                || operand_free_unknown_descriptor_operation_tag(failure.operation_tag())
                || failure.operation_tag() == 10
                || unknown_descriptor_open_at_tag(failure.operation_tag())
                || unknown_descriptor_unlink_at_tag(failure.operation_tag())
                || unknown_descriptor_read_dir_tag(failure.operation_tag())
                || unknown_descriptor_write_operation_tag(failure.operation_tag())
                || unknown_descriptor_set_file_times_tag(failure.operation_tag())
                || unknown_descriptor_read_operation_tag(failure.operation_tag())
                || unknown_descriptor_write_payload_operation_tag(failure.operation_tag())
                || unknown_descriptor_read_file_metadata_tag(failure.operation_tag())
                || unknown_descriptor_get_osfhandle_tag(failure.operation_tag())
                || unknown_native_handle_close_tag(failure.operation_tag())
                || unknown_native_handle_final_path_tag(failure.operation_tag())
                || unknown_native_handle_mutation_tag(failure.operation_tag())
        }
        [failure, error_read] => {
            ((unknown_native_handle_close_tag(failure.operation_tag())
                || unknown_native_handle_final_path_tag(failure.operation_tag())
                || unknown_native_handle_mutation_tag(failure.operation_tag()))
                && get_last_error_tag(error_read.operation_tag()))
                || (unknown_descriptor_bad_descriptor_failure_tag(failure.operation_tag())
                    && errno_tag(error_read.operation_tag()))
        }
        _ => false,
    }
}

pub(crate) fn source_input_replay_prefix_end(
    attempts: &[checked_interpreter::FilesystemOperationAttempt],
) -> Option<usize> {
    source_input_replay_prefix_refs(&attempts.iter().collect::<Vec<_>>())
}

fn source_input_replay_prefix_refs(
    attempts: &[&checked_interpreter::FilesystemOperationAttempt],
) -> Option<usize> {
    let mut cursor = 0;
    let mut identities = Vec::new();
    let mut event_count = 0;
    while cursor < attempts.len() {
        if matches!(
            attempts[cursor].operation_tag(),
            1 | 4
                | 5
                | 6
                | 7
                | 8
                | 9
                | 10
                | 11
                | 12
                | 14
                | 15
                | 17
                | 19
                | 20
                | 23
                | 27
                | 29
                | 30
                | 31
                | 32
                | 33
                | 34
                | 39
                | 41
                | 42
                | 43
                | 44
                | 45
                | 46
                | 49
        ) {
            break;
        }
        if attempts[cursor].operation_tag() == 21 {
            if !source_read_link_is_exact(attempts[cursor]) {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(attempts[cursor].operation_tag(), 38 | 40) {
            if !source_path_metadata_is_exact(attempts[cursor]) {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if attempts[cursor].operation_tag() == 28 {
            let identity = source_native_query_open_identity(attempts[cursor])?;
            if identities.contains(&identity) {
                return None;
            }
            identities.push(identity);
            cursor += 1;
            let mut saw_final_path_query = false;
            while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 31 | 35) {
                let operation_is_exact = if attempts[cursor].operation_tag() == 31 {
                    saw_final_path_query = true;
                    native_final_path_query_is_exact(attempts[cursor], identity)
                } else {
                    native_error_observation_is_exact(attempts[cursor])
                };
                if !operation_is_exact {
                    return None;
                }
                cursor += 1;
            }
            if !saw_final_path_query
                || cursor == attempts.len()
                || !source_native_query_close_is_exact(attempts[cursor], identity)
            {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        let identity = source_read_chain_open_identity(attempts[cursor])?;
        if identities.contains(&identity) {
            return None;
        }
        identities.push(identity);
        cursor += 1;

        if cursor < attempts.len() && attempts[cursor].operation_tag() == 39 {
            if !source_descriptor_metadata_is_exact(attempts[cursor], identity) {
                return None;
            }
            cursor += 1;
            if cursor == attempts.len()
                || !source_read_chain_close_is_exact(attempts[cursor], identity)
            {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }

        if cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
            let reads_start = cursor;
            while cursor < attempts.len() && attempts[cursor].operation_tag() == 23 {
                if !source_directory_read_is_exact(attempts[cursor], identity) {
                    return None;
                }
                cursor += 1;
            }
            if cursor == reads_start
                || cursor == attempts.len()
                || !source_read_chain_close_is_exact(attempts[cursor], identity)
            {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }

        let reads_start = cursor;
        while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 4 | 6) {
            if !source_read_chain_read_is_exact(attempts[cursor], identity) {
                return None;
            }
            cursor += 1;
        }
        if cursor == reads_start
            || cursor == attempts.len()
            || !source_read_chain_close_is_exact(attempts[cursor], identity)
        {
            return None;
        }
        cursor += 1;
        event_count += 1;
    }
    (event_count != 0
        || (cursor == 0
            && attempts.first().is_some_and(|attempt| {
                matches!(
                    attempt.operation_tag(),
                    1 | 4
                        | 5
                        | 6
                        | 7
                        | 8
                        | 9
                        | 10
                        | 11
                        | 12
                        | 14
                        | 15
                        | 17
                        | 19
                        | 20
                        | 23
                        | 27
                        | 29
                        | 30
                        | 31
                        | 32
                        | 33
                        | 34
                        | 39
                        | 41
                        | 42
                        | 43
                        | 44
                        | 45
                        | 46
                        | 49
                )
            })))
    .then_some(cursor)
}

fn source_read_link_is_exact(attempt: &checked_interpreter::FilesystemOperationAttempt) -> bool {
    use checked_interpreter::{
        FilesystemGrantAccess as Access, FilesystemObservationProvider as Provider,
        FilesystemOperationResult as ResultValue,
        FilesystemReturnedPathCompleteness as Completeness,
        FilesystemReturnedPathKind as ReturnedKind, FilesystemScalarOperandValue as ScalarValue,
    };
    let Some(ResultValue::Scalar(result)) = attempt.result() else {
        return false;
    };
    let Ok(result_length) = usize::try_from(result) else {
        return false;
    };
    let Ok(result_length_u64) = u64::try_from(result) else {
        return false;
    };
    let [count] = attempt.scalar_operands() else {
        return false;
    };
    let ScalarValue::U64(requested_count) = count.value() else {
        return false;
    };
    let Ok(requested_capacity) = usize::try_from(requested_count) else {
        return false;
    };
    let [rooted] = attempt.rooted_path_operand_resolutions() else {
        return false;
    };
    let [returned] = attempt.returned_paths() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands() else {
        return false;
    };
    let [authorized] = attempt.authorized_paths() else {
        return false;
    };
    let post_prefix_matches = mutable
        .post_bytes()
        .get(..result_length)
        .is_some_and(|prefix| prefix == returned.bytes());
    let unchanged_tail = mutable
        .pre_bytes()
        .get(result_length..)
        .zip(mutable.post_bytes().get(result_length..))
        .is_some_and(|(pre, post)| pre == post);
    let completeness_is_consistent = match returned.completeness() {
        Completeness::Complete => result_length_u64 <= requested_count,
        Completeness::LimitReached => result_length_u64 == requested_count,
    };
    attempt.operation_tag() == 21
        && attempt.provider() == Provider::RealScoped
        && count.operand_ordinal() == 2
        && result_length_u64 <= requested_count
        && requested_capacity <= mutable.pre_bytes().len()
        && result_length <= mutable.post_bytes().len()
        && attempt.byte_operands().is_empty()
        && attempt.path_like_operands().is_empty()
        && rooted.operand_ordinal() == 0
        && rooted.root() == BUILD_SOURCE_ROOT_IDENTITY
        && checked_interpreter::filesystem_root_relative_path_is_canonical(
            rooted.relative_path(),
            false,
        )
        && returned.operand_ordinal() == 1
        && returned.kind() == ReturnedKind::ReadLinkPayload
        && returned.bytes().len() == result_length
        && completeness_is_consistent
        && attempt.observed_byte_regions().is_empty()
        && attempt.metadata_observations().is_empty()
        && mutable_resolution.operand_ordinal() == 1
        && mutable.operand_ordinal() == 1
        && mutable_resolution.bytes() == mutable.pre_bytes()
        && mutable.pre_bytes().len() == mutable.post_bytes().len()
        && post_prefix_matches
        && unchanged_tail
        && attempt.mutable_i64_operand_resolutions().is_empty()
        && attempt.mutable_i64_operands().is_empty()
        && authorized.operand_ordinal() == 0
        && authorized.access() == Access::Read
        && authorized.root() == BUILD_SOURCE_ROOT_IDENTITY
        && checked_interpreter::filesystem_root_relative_path_is_canonical(
            authorized.relative_path(),
            true,
        )
        && attempt.logical_handle_inputs().is_empty()
        && attempt.logical_handle_output().is_none()
        && attempt.retired_logical_handles().is_empty()
        && attempt.grant_refusals().is_empty()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReceiptedOutputFile {
    pub(crate) relative_path: Vec<u8>,
    pub(crate) bytes: Vec<u8>,
    pub(crate) executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReceiptedOutputEntry {
    Directory {
        relative_path: Vec<u8>,
    },
    File(ReceiptedOutputFile),
    HardLink {
        existing_relative_path: Vec<u8>,
        relative_path: Vec<u8>,
    },
    Symlink {
        relative_path: Vec<u8>,
        target_spelling: Vec<u8>,
    },
}

pub(crate) fn receipted_output_entries(
    replay: &checked_interpreter::FilesystemReplay,
) -> Option<Vec<ReceiptedOutputEntry>> {
    let entries = replay.output_entries();
    if entries.is_empty() {
        return replay.has_output_attempts().then(Vec::new);
    }
    entries
        .into_iter()
        .map(|entry| match entry {
            checked_interpreter::FilesystemOutputTreeEntryReplayRecord::Directory(directory) => {
                (directory.output_root() == BUILD_OUTPUT_ROOT_IDENTITY
                    && directory.mode()
                        == checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE
                    && directory.result() == 0
                    && directory.post_error() == 0)
                    .then(|| ReceiptedOutputEntry::Directory {
                        relative_path: directory.output_relative_path().to_vec(),
                    })
            }
            checked_interpreter::FilesystemOutputTreeEntryReplayRecord::File(output) => {
                if output.output_root() != BUILD_OUTPUT_ROOT_IDENTITY
                    || output.create_post_error() != 0
                    || output.close_post_error() != 0
                {
                    return None;
                }
                let bytes = output.replayed_bytes().ok()?;
                Some(ReceiptedOutputEntry::File(ReceiptedOutputFile {
                    relative_path: output.output_relative_path().to_vec(),
                    bytes,
                    executable: output.replayed_executable(),
                }))
            }
            checked_interpreter::FilesystemOutputTreeEntryReplayRecord::HardLink(hard_link) => {
                (hard_link.output_root() == BUILD_OUTPUT_ROOT_IDENTITY
                    && hard_link.post_error() == 0
                    && matches!(hard_link.result(), 0 | 1))
                .then(|| ReceiptedOutputEntry::HardLink {
                    existing_relative_path: hard_link.existing_relative_path().to_vec(),
                    relative_path: hard_link.output_relative_path().to_vec(),
                })
            }
            checked_interpreter::FilesystemOutputTreeEntryReplayRecord::Symlink(symlink) => {
                (symlink.output_root() == BUILD_OUTPUT_ROOT_IDENTITY
                    && symlink.result() == 0
                    && symlink.post_error() == 0)
                    .then(|| ReceiptedOutputEntry::Symlink {
                        relative_path: symlink.output_relative_path().to_vec(),
                        target_spelling: symlink.target_spelling().to_vec(),
                    })
            }
        })
        .collect()
}

fn source_path_metadata_is_exact(
    attempt: &checked_interpreter::FilesystemOperationAttempt,
) -> bool {
    use checked_interpreter::{
        FilesystemGrantAccess as Access, FilesystemMetadataObservationKind as MetadataKind,
        FilesystemObservationProvider as Provider, FilesystemOperationResult as ResultValue,
    };
    let expected_kind = match attempt.operation_tag() {
        38 => MetadataKind::FollowedPath,
        40 => MetadataKind::UnfollowedFinalPath,
        _ => return false,
    };
    let [rooted] = attempt.rooted_path_operand_resolutions() else {
        return false;
    };
    let [authorized] = attempt.authorized_paths() else {
        return false;
    };
    let [metadata] = attempt.metadata_observations() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands() else {
        return false;
    };
    attempt.provider() == Provider::RealScoped
        && attempt.result() == Some(ResultValue::Scalar(0))
        && attempt.scalar_operands().is_empty()
        && attempt.byte_operands().is_empty()
        && attempt.path_like_operands().is_empty()
        && rooted.operand_ordinal() == 0
        && rooted.root() == BUILD_SOURCE_ROOT_IDENTITY
        && checked_interpreter::filesystem_root_relative_path_is_canonical(
            rooted.relative_path(),
            false,
        )
        && attempt.returned_paths().is_empty()
        && attempt.observed_byte_regions().is_empty()
        && authorized.operand_ordinal() == 0
        && authorized.access() == Access::Read
        && authorized.root() == BUILD_SOURCE_ROOT_IDENTITY
        && checked_interpreter::filesystem_root_relative_path_is_canonical(
            authorized.relative_path(),
            true,
        )
        && metadata.output_operand_ordinal() == 1
        && metadata.kind() == expected_kind
        && mutable_resolution.operand_ordinal() == 1
        && mutable.operand_ordinal() == 1
        && mutable_resolution.bytes() == mutable.pre_bytes()
        && mutable.pre_bytes().len() == mutable.post_bytes().len()
        && mutable.post_bytes().len() >= checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        && attempt.mutable_i64_operand_resolutions().is_empty()
        && attempt.mutable_i64_operands().is_empty()
        && attempt.logical_handle_inputs().is_empty()
        && attempt.logical_handle_output().is_none()
        && attempt.retired_logical_handles().is_empty()
        && attempt.grant_refusals().is_empty()
}

fn source_read_chain_open_identity(
    open: &checked_interpreter::FilesystemOperationAttempt,
) -> Option<checked_interpreter::FilesystemLogicalHandleIdentity> {
    use checked_interpreter::{
        FilesystemLogicalHandleKind as HandleKind,
        FilesystemLogicalHandleOutputSource as OutputSource,
        FilesystemOperationResult as ResultValue, FilesystemScalarOperandValue as ScalarValue,
    };
    let [rooted] = open.rooted_path_operand_resolutions() else {
        return None;
    };
    let [flags] = open.scalar_operands() else {
        return None;
    };
    let output = open.logical_handle_output()?;
    let identity = output.identity();
    (open.operation_tag() == 2
        && open.provider() == checked_interpreter::FilesystemObservationProvider::RealScoped
        && rooted.operand_ordinal() == 0
        && rooted.root() == BUILD_SOURCE_ROOT_IDENTITY
        && flags.operand_ordinal() == 1
        && flags.value() == ScalarValue::I32(0)
        && output.kind() == HandleKind::Descriptor
        && output.source() == OutputSource::Created
        && open.result() == Some(ResultValue::LogicalHandle(identity)))
    .then_some(identity)
}

fn source_read_chain_read_is_exact(
    read: &checked_interpreter::FilesystemOperationAttempt,
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> bool {
    use checked_interpreter::{
        FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind, FilesystemOperationResult as ResultValue,
        FilesystemScalarOperandValue as ScalarValue,
    };
    let [read_input] = read.logical_handle_inputs() else {
        return false;
    };
    let Some(ResultValue::Scalar(read_result)) = read.result() else {
        return false;
    };
    let Ok(read_length) = usize::try_from(read_result) else {
        return false;
    };
    let Ok(read_length_u64) = u64::try_from(read_result) else {
        return false;
    };
    let expected_region_kind = match (read.operation_tag(), read.scalar_operands()) {
        (4, [count])
            if count.operand_ordinal() == 2
                && matches!(count.value(), ScalarValue::U64(requested) if requested >= read_length_u64) =>
        {
            checked_interpreter::FilesystemObservedByteRegionKind::SequentialFileRead
        }
        (6, [count, offset])
            if count.operand_ordinal() == 2
                && matches!(count.value(), ScalarValue::U64(requested) if requested >= read_length_u64)
                && offset.operand_ordinal() == 3
                && matches!(offset.value(), ScalarValue::I64(value) if value >= 0) =>
        {
            checked_interpreter::FilesystemObservedByteRegionKind::PositionedFileRead
        }
        _ => return false,
    };
    let [region] = read.observed_byte_regions() else {
        return false;
    };
    read.provider() == checked_interpreter::FilesystemObservationProvider::RealScoped
        && read_input.operand_ordinal() == 0
        && read_input.kind() == HandleKind::Descriptor
        && read_input.resolution() == InputResolution::Resolved(identity)
        && region.output_operand_ordinal() == 1
        && region.kind() == expected_region_kind
        && region.offset() == 0
        && region.length() == read_length
}

fn source_directory_read_is_exact(
    read: &checked_interpreter::FilesystemOperationAttempt,
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> bool {
    use checked_interpreter::{
        FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind, FilesystemObservationProvider as Provider,
        FilesystemObservedByteRegionKind as RegionKind, FilesystemOperationResult as ResultValue,
        FilesystemScalarOperandValue as ScalarValue,
    };
    let Some(ResultValue::Scalar(result)) = read.result() else {
        return false;
    };
    let Ok(result_length) = usize::try_from(result) else {
        return false;
    };
    let [count] = read.scalar_operands() else {
        return false;
    };
    let ScalarValue::U64(requested_count) = count.value() else {
        return false;
    };
    let Ok(requested_capacity) = usize::try_from(requested_count) else {
        return false;
    };
    let [region] = read.observed_byte_regions() else {
        return false;
    };
    let [resolution] = read.mutable_byte_operand_resolutions() else {
        return false;
    };
    let [mutable] = read.mutable_byte_operands() else {
        return false;
    };
    let [position_resolution] = read.mutable_i64_operand_resolutions() else {
        return false;
    };
    let [position] = read.mutable_i64_operands() else {
        return false;
    };
    let [handle] = read.logical_handle_inputs() else {
        return false;
    };
    read.operation_tag() == 23
        && read.provider() == Provider::RealScoped
        && count.operand_ordinal() == 2
        && result_length <= requested_capacity
        && region.output_operand_ordinal() == 1
        && region.kind() == RegionKind::DirectoryRecords
        && region.offset() == 0
        && region.length() == result_length
        && resolution.operand_ordinal() == 1
        && mutable.operand_ordinal() == 1
        && resolution.bytes().len() == mutable.pre_bytes().len()
        && mutable.pre_bytes().len() == mutable.post_bytes().len()
        && requested_capacity <= mutable.post_bytes().len()
        && mutable.pre_bytes()[result_length..] == mutable.post_bytes()[result_length..]
        && position_resolution.operand_ordinal() == 3
        && position.operand_ordinal() == 3
        && handle.operand_ordinal() == 0
        && handle.kind() == HandleKind::Descriptor
        && handle.resolution() == InputResolution::Resolved(identity)
        && read.byte_operands().is_empty()
        && read.path_like_operands().is_empty()
        && read.rooted_path_operand_resolutions().is_empty()
        && read.returned_paths().is_empty()
        && read.metadata_observations().is_empty()
        && read.authorized_paths().is_empty()
        && read.logical_handle_output().is_none()
        && read.retired_logical_handles().is_empty()
        && read.grant_refusals().is_empty()
}

fn source_descriptor_metadata_is_exact(
    attempt: &checked_interpreter::FilesystemOperationAttempt,
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> bool {
    use checked_interpreter::{
        FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind,
        FilesystemMetadataObservationKind as MetadataKind,
        FilesystemObservationProvider as Provider, FilesystemOperationResult as ResultValue,
    };
    let [descriptor] = attempt.logical_handle_inputs() else {
        return false;
    };
    let [metadata] = attempt.metadata_observations() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands() else {
        return false;
    };
    attempt.operation_tag() == 39
        && attempt.provider() == Provider::RealScoped
        && attempt.result() == Some(ResultValue::Scalar(0))
        && descriptor.operand_ordinal() == 0
        && descriptor.kind() == HandleKind::Descriptor
        && descriptor.resolution() == InputResolution::Resolved(identity)
        && metadata.output_operand_ordinal() == 1
        && metadata.kind() == MetadataKind::OpenDescriptor
        && mutable_resolution.operand_ordinal() == 1
        && mutable.operand_ordinal() == 1
        && mutable_resolution.bytes() == mutable.pre_bytes()
        && mutable.pre_bytes().len() == mutable.post_bytes().len()
        && mutable.post_bytes().len() >= checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        && attempt.scalar_operands().is_empty()
        && attempt.byte_operands().is_empty()
        && attempt.path_like_operands().is_empty()
        && attempt.rooted_path_operand_resolutions().is_empty()
        && attempt.returned_paths().is_empty()
        && attempt.observed_byte_regions().is_empty()
        && attempt.mutable_i64_operand_resolutions().is_empty()
        && attempt.mutable_i64_operands().is_empty()
        && attempt.authorized_paths().is_empty()
        && attempt.logical_handle_output().is_none()
        && attempt.retired_logical_handles().is_empty()
        && attempt.grant_refusals().is_empty()
}

fn source_read_chain_close_is_exact(
    close: &checked_interpreter::FilesystemOperationAttempt,
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> bool {
    use checked_interpreter::{
        FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind, FilesystemOperationResult as ResultValue,
    };
    let [close_input] = close.logical_handle_inputs() else {
        return false;
    };
    close.operation_tag() == 8
        && close.provider() == checked_interpreter::FilesystemObservationProvider::RealScoped
        && close_input.operand_ordinal() == 0
        && close_input.kind() == HandleKind::Descriptor
        && close_input.resolution() == InputResolution::Resolved(identity)
        && close.result() == Some(ResultValue::Scalar(0))
        && close.retired_logical_handles() == [identity]
}

/// Identity acquired by one constrained Source `open_path_handle` (tag 28)
/// under the bounded query-only contract, or `None` when the attempt is not
/// the exact acquisition: access zero, full read/write/delete sharing, null
/// security attributes, `OPEN_EXISTING`, `FILE_FLAG_BACKUP_SEMANTICS` without
/// `FILE_FLAG_DELETE_ON_CLOSE`, and a null template handle input.
fn source_native_query_open_identity(
    open: &checked_interpreter::FilesystemOperationAttempt,
) -> Option<checked_interpreter::FilesystemLogicalHandleIdentity> {
    use checked_interpreter::{
        FilesystemGrantAccess as Access, FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind,
        FilesystemLogicalHandleOutputSource as OutputSource,
        FilesystemObservationProvider as Provider, FilesystemOperationResult as ResultValue,
        FilesystemScalarOperandValue as ScalarValue,
    };
    let Some(ResultValue::LogicalHandle(identity)) = open.result() else {
        return None;
    };
    let [
        desired_access,
        share_mode,
        security_attributes,
        creation_disposition,
        flags,
    ] = open.scalar_operands()
    else {
        return None;
    };
    let [rooted] = open.rooted_path_operand_resolutions() else {
        return None;
    };
    let [authorized] = open.authorized_paths() else {
        return None;
    };
    let [template] = open.logical_handle_inputs() else {
        return None;
    };
    let output = open.logical_handle_output()?;
    (open.operation_tag() == 28
        && open.provider() == Provider::RealScoped
        && desired_access.operand_ordinal() == 1
        && desired_access.value() == ScalarValue::U32(0)
        && share_mode.operand_ordinal() == 2
        && share_mode.value() == ScalarValue::U32(0x7)
        && security_attributes.operand_ordinal() == 3
        && security_attributes.value() == ScalarValue::I64(0)
        && creation_disposition.operand_ordinal() == 4
        && creation_disposition.value() == ScalarValue::U32(3)
        && flags.operand_ordinal() == 5
        && flags.value() == ScalarValue::U32(0x0200_0000)
        && rooted.operand_ordinal() == 0
        && rooted.root() == BUILD_SOURCE_ROOT_IDENTITY
        && checked_interpreter::filesystem_root_relative_path_is_canonical(
            rooted.relative_path(),
            false,
        )
        && authorized.operand_ordinal() == 0
        && authorized.access() == Access::Read
        && authorized.root() == BUILD_SOURCE_ROOT_IDENTITY
        && authorized.relative_path() == rooted.relative_path()
        && template.operand_ordinal() == 6
        && template.kind() == HandleKind::Native
        && template.resolution() == InputResolution::Null
        && output.kind() == HandleKind::Native
        && output.identity() == identity
        && output.source() == OutputSource::Created
        && open.byte_operands().is_empty()
        && open.path_like_operands().is_empty()
        && open.returned_paths().is_empty()
        && open.observed_byte_regions().is_empty()
        && open.metadata_observations().is_empty()
        && open.mutable_byte_operand_resolutions().is_empty()
        && open.mutable_i64_operand_resolutions().is_empty()
        && open.mutable_byte_operands().is_empty()
        && open.mutable_i64_operands().is_empty()
        && open.retired_logical_handles().is_empty()
        && open.grant_refusals().is_empty())
    .then_some(identity)
}

/// Whether one `final_path_name_by_handle` (tag 31) attempt is an exact
/// handle-preserving observation on `identity` under the bounded
/// query-release contract. Buffer custody, the returned path, and the result
/// are rechecked through the checked-interpreter record constructor so the
/// eligibility mirror cannot drift from the replay grammar.
fn native_final_path_query_is_exact(
    query: &checked_interpreter::FilesystemOperationAttempt,
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> bool {
    use checked_interpreter::{
        FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind, FilesystemObservationProvider as Provider,
        FilesystemOperationResult as ResultValue,
        FilesystemReturnedPathCompleteness as Completeness,
        FilesystemReturnedPathKind as ReturnedKind, FilesystemScalarOperandValue as ScalarValue,
    };
    let Some(ResultValue::Scalar(result)) = query.result() else {
        return false;
    };
    let Some(post_error) = query.post_error() else {
        return false;
    };
    let [capacity, flags] = query.scalar_operands() else {
        return false;
    };
    let ScalarValue::U64(capacity_value) = capacity.value() else {
        return false;
    };
    let ScalarValue::U32(flags_value) = flags.value() else {
        return false;
    };
    let [returned] = query.returned_paths() else {
        return false;
    };
    let [resolution] = query.mutable_byte_operand_resolutions() else {
        return false;
    };
    let [mutable] = query.mutable_byte_operands() else {
        return false;
    };
    let [handle] = query.logical_handle_inputs() else {
        return false;
    };
    query.operation_tag() == 31
        && query.provider() == Provider::RealScoped
        && capacity.operand_ordinal() == 2
        && flags.operand_ordinal() == 3
        && returned.operand_ordinal() == 1
        && returned.kind() == ReturnedKind::FinalPath
        && returned.completeness() == Completeness::Complete
        && resolution.operand_ordinal() == 1
        && mutable.operand_ordinal() == 1
        && handle.operand_ordinal() == 0
        && handle.kind() == HandleKind::Native
        && handle.resolution() == InputResolution::Resolved(identity)
        && checked_interpreter::FilesystemNativeHandleFinalPathQueryReplayRecord::new(
            capacity_value,
            flags_value,
            result,
            post_error,
            resolution.bytes().to_vec(),
            mutable.pre_bytes().to_vec(),
            mutable.post_bytes().to_vec(),
            returned.bytes().to_vec(),
        )
        .is_ok()
        && query.byte_operands().is_empty()
        && query.path_like_operands().is_empty()
        && query.rooted_path_operand_resolutions().is_empty()
        && query.observed_byte_regions().is_empty()
        && query.metadata_observations().is_empty()
        && query.mutable_i64_operand_resolutions().is_empty()
        && query.mutable_i64_operands().is_empty()
        && query.authorized_paths().is_empty()
        && query.logical_handle_output().is_none()
        && query.retired_logical_handles().is_empty()
        && query.grant_refusals().is_empty()
}

/// Whether one `get_last_error` (tag 35) attempt is an exact handle-free
/// error-slot observation inside a bounded query-release chain.
fn native_error_observation_is_exact(
    operation: &checked_interpreter::FilesystemOperationAttempt,
) -> bool {
    use checked_interpreter::{
        FilesystemObservationProvider as Provider, FilesystemOperationResult as ResultValue,
    };
    let Some(post_error) = operation.post_error() else {
        return false;
    };
    operation.operation_tag() == 35
        && operation.provider() == Provider::RealScoped
        && operation.result() == Some(ResultValue::Scalar(i64::from(post_error)))
        && operation.scalar_operands().is_empty()
        && operation.byte_operands().is_empty()
        && operation.path_like_operands().is_empty()
        && operation.rooted_path_operand_resolutions().is_empty()
        && operation.returned_paths().is_empty()
        && operation.observed_byte_regions().is_empty()
        && operation.metadata_observations().is_empty()
        && operation.mutable_byte_operand_resolutions().is_empty()
        && operation.mutable_i64_operand_resolutions().is_empty()
        && operation.mutable_byte_operands().is_empty()
        && operation.mutable_i64_operands().is_empty()
        && operation.authorized_paths().is_empty()
        && operation.logical_handle_inputs().is_empty()
        && operation.logical_handle_output().is_none()
        && operation.retired_logical_handles().is_empty()
        && operation.grant_refusals().is_empty()
}

/// Whether one `close_handle` (tag 29) attempt is the exact successful
/// release that retires `identity` at the end of a bounded query-release
/// chain.
fn source_native_query_close_is_exact(
    close: &checked_interpreter::FilesystemOperationAttempt,
    identity: checked_interpreter::FilesystemLogicalHandleIdentity,
) -> bool {
    use checked_interpreter::{
        FilesystemLogicalHandleInputResolution as InputResolution,
        FilesystemLogicalHandleKind as HandleKind, FilesystemObservationProvider as Provider,
        FilesystemOperationResult as ResultValue,
    };
    let Some(ResultValue::Scalar(result)) = close.result() else {
        return false;
    };
    let [handle] = close.logical_handle_inputs() else {
        return false;
    };
    close.operation_tag() == 29
        && close.provider() == Provider::RealScoped
        && result != 0
        && handle.operand_ordinal() == 0
        && handle.kind() == HandleKind::Native
        && handle.resolution() == InputResolution::Resolved(identity)
        && close.retired_logical_handles() == [identity]
        && close.scalar_operands().is_empty()
        && close.byte_operands().is_empty()
        && close.path_like_operands().is_empty()
        && close.rooted_path_operand_resolutions().is_empty()
        && close.returned_paths().is_empty()
        && close.observed_byte_regions().is_empty()
        && close.metadata_observations().is_empty()
        && close.mutable_byte_operand_resolutions().is_empty()
        && close.mutable_i64_operand_resolutions().is_empty()
        && close.mutable_byte_operands().is_empty()
        && close.mutable_i64_operands().is_empty()
        && close.authorized_paths().is_empty()
        && close.logical_handle_output().is_none()
        && close.grant_refusals().is_empty()
}
