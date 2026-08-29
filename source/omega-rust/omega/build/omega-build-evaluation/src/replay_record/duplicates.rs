use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeLogicalInput, ShapeResult,
    validate_close_shape,
};

pub(super) fn validate_output_duplicate_shapes(
    duplicate: &AttemptShape<'_>,
    close: &AttemptShape<'_>,
    source_identity: u64,
) -> Result<u64, BuildFilesystemReplayRecordError> {
    let [input] = duplicate.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output duplicate has no unique source",
        ));
    };
    let Some(output) = duplicate.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output duplicate has no fresh identity",
        ));
    };
    if duplicate.operation != 45
        || duplicate.provider != 2
        || duplicate.result != ShapeResult::Handle(output.identity)
        || duplicate.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: Some(source_identity),
            })
        || output.kind != 0
        || output.source != 1
        || output.source_identity != Some(source_identity)
        || !only_output_duplicate_lanes(duplicate)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output duplicate is internally inconsistent",
        ));
    }
    validate_close_shape(close, output.identity)?;
    if close.post_error != 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output duplicate close changed the post-operation error state",
        ));
    }
    Ok(output.identity)
}

fn only_output_duplicate_lanes(attempt: &AttemptShape<'_>) -> bool {
    attempt.scalars.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operand_count == 0
        && attempt.rooted_paths.is_empty()
        && attempt.returned_path_count == 0
        && attempt.observed_regions.is_empty()
        && attempt.metadata.is_empty()
        && attempt.mutable_byte_resolutions.is_empty()
        && attempt.mutable_i64_resolution_count == 0
        && attempt.mutable_bytes.is_empty()
        && attempt.mutable_i64_count == 0
        && attempt.authorized_paths.is_empty()
        && attempt.retired.is_empty()
        && attempt.refusal_count == 0
}
