use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeLogicalInput, ShapeResult, ShapeScalar,
};

const LOCK_FILE_OPERATION_TAG: u16 = 46;
const NON_BLOCKING_EXCLUSIVE_LOCK: i32 = 6;
const UNLOCK: i32 = 8;

pub(super) fn validate_output_lock_shapes(
    acquire: &AttemptShape<'_>,
    release: &AttemptShape<'_>,
    descriptor_identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    validate_lock_shape(
        acquire,
        descriptor_identity,
        NON_BLOCKING_EXCLUSIVE_LOCK,
        "acquire",
    )?;
    validate_lock_shape(release, descriptor_identity, UNLOCK, "release")
}

fn validate_lock_shape(
    attempt: &AttemptShape<'_>,
    descriptor_identity: u64,
    expected_operation: i32,
    phase: &'static str,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [input] = attempt.inputs.as_slice() else {
        return Err(lock_error(phase));
    };
    if attempt.operation != LOCK_FILE_OPERATION_TAG
        || attempt.provider != 2
        || attempt.result != ShapeResult::Scalar(0)
        || attempt.post_error != 0
        || attempt.scalars.as_slice() != [(1, ShapeScalar::I32(expected_operation))]
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: Some(descriptor_identity),
            })
        || !only_output_lock_lanes(attempt)
    {
        return Err(lock_error(phase));
    }
    Ok(())
}

fn lock_error(phase: &'static str) -> BuildFilesystemReplayRecordError {
    BuildFilesystemReplayRecordError::new(match phase {
        "acquire" => "receipted build output lock acquire is internally inconsistent",
        "release" => "receipted build output lock release is internally inconsistent",
        _ => "receipted build output lock is internally inconsistent",
    })
}

fn only_output_lock_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64_count == 0
        && attempt.authorized_paths.is_empty()
        && attempt.output.is_none()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}
