//! Which lanes an attempt of each kind may populate, and the helpers that
//! assert every other lane is empty.

use crate::replay_record::attempt_codec::AttemptShape;

fn common_empty_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_path_metadata_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.scalars.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_descriptor_metadata_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.scalars.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_open_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.inputs.is_empty()
        && attempt.retired.is_empty()
}

pub(crate) fn only_native_query_open_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.retired.is_empty()
}

pub(crate) fn only_native_final_path_query_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_native_error_observation_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.scalars.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

pub(crate) fn only_read_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

pub(crate) fn only_directory_read_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_close_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.scalars.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.output.is_none()
}

pub(crate) fn only_output_create_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.inputs.is_empty()
        && attempt.retired.is_empty()
}

pub(crate) fn only_output_absent_remove_lanes(attempt: &AttemptShape<'_>) -> bool {
    common_empty_lanes(attempt)
        && attempt.scalars.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

pub(crate) fn only_output_write_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.path_like_operands.is_empty()
        && attempt.returned_path_count == 0
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.refusal_count == 0
        && attempt.rooted_paths.is_empty()
        && attempt.observed_regions.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
}

pub(crate) fn only_output_sync_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.scalars.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_output_set_length_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_output_set_file_permissions_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_output_set_file_times_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.scalars.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

pub(crate) fn only_output_seek_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolutions.is_empty()
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64s.is_empty()
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}
