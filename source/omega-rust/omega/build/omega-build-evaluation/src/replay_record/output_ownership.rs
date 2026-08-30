use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeLogicalInput, ShapeLogicalInputResolution,
    ShapeResult, ShapeScalar,
};

const CHANGE_FILE_OWNER_OPERATION_TAG: u16 = 49;

pub(super) fn validate_output_change_file_owner_shape(
    operation: &AttemptShape<'_>,
    descriptor_identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::I32(_uid)), (2, ShapeScalar::I32(_gid))] = operation.scalars.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output change_file_owner has no exact uid and gid",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output change_file_owner has no unique descriptor input",
        ));
    };
    if operation.operation != CHANGE_FILE_OWNER_OPERATION_TAG
        || operation.provider != 2
        || !matches!(operation.result, ShapeResult::Scalar(-1 | 0))
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(descriptor_identity),
            })
        || !only_output_change_file_owner_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output change_file_owner is internally inconsistent",
        ));
    }
    Ok(())
}

fn only_output_change_file_owner_lanes(attempt: &AttemptShape<'_>) -> bool {
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
