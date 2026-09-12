//! Supported filesystem replay sequences and retained Output reconstruction.

use crate::{BUILD_OUTPUT_ROOT_IDENTITY, BUILD_SOURCE_ROOT_IDENTITY};

pub(super) fn is_source_input_replay_record(
    observations: &checked_interpreter::EvaluationObservations,
) -> bool {
    let attempts = observations.filesystem_operation_attempts();
    source_input_replay_prefix_end(attempts) == Some(attempts.len())
}

pub(super) const fn operand_free_unknown_descriptor_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 8 | 43 | 44 | 45)
}

pub(super) const fn unknown_descriptor_write_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 17 | 41 | 46 | 49)
}

pub(super) const fn unknown_descriptor_set_file_times_tag(operation_tag: u16) -> bool {
    operation_tag == 42
}

pub(super) const fn unknown_descriptor_read_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 4 | 6)
}

pub(super) const fn unknown_descriptor_open_at_tag(operation_tag: u16) -> bool {
    operation_tag == 14
}

pub(super) const fn unknown_descriptor_unlink_at_tag(operation_tag: u16) -> bool {
    operation_tag == 15
}

pub(super) const fn unknown_descriptor_read_dir_tag(operation_tag: u16) -> bool {
    operation_tag == 23
}

pub(super) const fn unknown_descriptor_write_payload_operation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 5 | 7)
}

pub(super) const fn unknown_descriptor_read_file_metadata_tag(operation_tag: u16) -> bool {
    operation_tag == 39
}

pub(super) const fn unknown_descriptor_get_osfhandle_tag(operation_tag: u16) -> bool {
    operation_tag == 30
}

pub(super) const fn unknown_native_handle_close_tag(operation_tag: u16) -> bool {
    operation_tag == 29
}

pub(super) const fn unknown_native_handle_final_path_tag(operation_tag: u16) -> bool {
    operation_tag == 31
}

pub(super) const fn unknown_native_handle_mutation_tag(operation_tag: u16) -> bool {
    matches!(operation_tag, 32..=34)
}

pub(super) const fn get_last_error_tag(operation_tag: u16) -> bool {
    operation_tag == 35
}

pub(super) const fn errno_tag(operation_tag: u16) -> bool {
    operation_tag == 50
}

pub(super) const fn unknown_descriptor_bad_descriptor_failure_tag(operation_tag: u16) -> bool {
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

pub(super) fn exact_source_write_refusal(
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

pub(super) fn complete_no_output_failure_suffix_is_recognized(
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

pub(super) fn source_input_replay_prefix_end(
    attempts: &[checked_interpreter::FilesystemOperationAttempt],
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
            if !source_read_link_is_exact(&attempts[cursor]) {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(attempts[cursor].operation_tag(), 38 | 40) {
            if !source_path_metadata_is_exact(&attempts[cursor]) {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }
        let identity = source_read_chain_open_identity(&attempts[cursor])?;
        if identities.contains(&identity) {
            return None;
        }
        identities.push(identity);
        cursor += 1;

        if cursor < attempts.len() && attempts[cursor].operation_tag() == 39 {
            if !source_descriptor_metadata_is_exact(&attempts[cursor], identity) {
                return None;
            }
            cursor += 1;
            if cursor == attempts.len()
                || !source_read_chain_close_is_exact(&attempts[cursor], identity)
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
                if !source_directory_read_is_exact(&attempts[cursor], identity) {
                    return None;
                }
                cursor += 1;
            }
            if cursor == reads_start
                || cursor == attempts.len()
                || !source_read_chain_close_is_exact(&attempts[cursor], identity)
            {
                return None;
            }
            cursor += 1;
            event_count += 1;
            continue;
        }

        let reads_start = cursor;
        while cursor < attempts.len() && matches!(attempts[cursor].operation_tag(), 4 | 6) {
            if !source_read_chain_read_is_exact(&attempts[cursor], identity) {
                return None;
            }
            cursor += 1;
        }
        if cursor == reads_start
            || cursor == attempts.len()
            || !source_read_chain_close_is_exact(&attempts[cursor], identity)
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
pub(super) struct ReceiptedOutputFile {
    pub(super) relative_path: Vec<u8>,
    pub(super) bytes: Vec<u8>,
    pub(super) executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReceiptedOutputEntry {
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

pub(super) fn receipted_output_entries(
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
