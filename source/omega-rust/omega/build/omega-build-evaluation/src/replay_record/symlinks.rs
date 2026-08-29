use super::{AttemptShape, BuildFilesystemReplayRecordError, ShapeResult, clone_bytes};

const SYMLINK_OPERATION_TAG: u16 = 20;

pub(super) fn validate_output_symlink_shape(
    attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(target_ordinal, target)] = attempt.path_like_operands.as_slice() else {
        return Err(symlink_shape_error());
    };
    let [rooted] = attempt.rooted_paths.as_slice() else {
        return Err(symlink_shape_error());
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return Err(symlink_shape_error());
    };
    if attempt.operation != SYMLINK_OPERATION_TAG
        || attempt.provider != 2
        || attempt.result != ShapeResult::Scalar(0)
        || attempt.post_error != 0
        || !attempt.scalars.is_empty()
        || !attempt.byte_operands.is_empty()
        || *target_ordinal != 0
        || target.is_empty()
        || target.len() > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES
        || target.contains(&0)
        || rooted.ordinal != 1
        || rooted.root != 1
        || rooted.bytes.len()
            > psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
        || !psi_checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 1
        || authorized.access != 1
        || authorized.root != 1
        || authorized.bytes != rooted.bytes
        || !only_symlink_lanes(attempt)
    {
        return Err(symlink_shape_error());
    }
    Ok(())
}

pub(super) fn rehydrate_output_symlink_shape(
    attempt: &AttemptShape<'_>,
) -> Result<
    psi_checked_interpreter::FilesystemOutputSymlinkReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let [(_, target)] = attempt.path_like_operands.as_slice() else {
        unreachable!("validated Output symlink has one target spelling")
    };
    let [rooted] = attempt.rooted_paths.as_slice() else {
        unreachable!("validated Output symlink has one rooted link path")
    };
    psi_checked_interpreter::FilesystemOutputSymlinkReplayRecord::new(
        crate::BUILD_OUTPUT_ROOT_IDENTITY,
        clone_bytes(rooted.bytes)?,
        clone_bytes(target)?,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Output symlink could not be rehydrated",
        )
    })
}

fn only_symlink_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.returned_path_count == 0
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

fn symlink_shape_error() -> BuildFilesystemReplayRecordError {
    BuildFilesystemReplayRecordError::new(
        "receipted build output symlink is internally inconsistent",
    )
}
