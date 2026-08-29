use super::{AttemptShape, BuildFilesystemReplayRecordError, ShapeResult, ShapeScalar};

const CREATE_DIRECTORY_OPERATION_TAG: u16 = 11;

pub(super) fn validate_output_directory_shape(
    attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [rooted] = attempt.rooted_paths.as_slice() else {
        return Err(directory_shape_error());
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return Err(directory_shape_error());
    };
    if attempt.operation != CREATE_DIRECTORY_OPERATION_TAG
        || attempt.provider != 2
        || attempt.result != ShapeResult::Scalar(0)
        || attempt.post_error != 0
        || attempt.scalars.as_slice()
            != [(
                1,
                ShapeScalar::I32(psi_checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE),
            )]
        || rooted.ordinal != 0
        || rooted.root != 1
        || rooted.bytes.len()
            > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
        || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 1
        || authorized.root != 1
        || authorized.bytes != rooted.bytes
        || !only_directory_lanes(attempt)
    {
        return Err(directory_shape_error());
    }
    Ok(())
}

fn only_directory_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operand_count == 0
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64_count == 0
        && attempt.inputs.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}

fn directory_shape_error() -> BuildFilesystemReplayRecordError {
    BuildFilesystemReplayRecordError::new(
        "receipted build output directory creation is internally inconsistent",
    )
}
