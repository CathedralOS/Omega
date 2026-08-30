//! Exact failed logical-handle operations retained by build replay records.

use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeLogicalInput, ShapeLogicalInputResolution,
    ShapeResult,
};

const CLOSE_OPERATION_TAG: u16 = 8;
const REAL_SCOPED_PROVIDER_TAG: u8 = 2;
const DESCRIPTOR_HANDLE_KIND_TAG: u8 = 0;
const BAD_DESCRIPTOR_RESULT: i64 = -1;
const BAD_DESCRIPTOR_ERROR: i32 = 9;

pub(super) fn unknown_descriptor_close_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.operation == CLOSE_OPERATION_TAG
        && shape.provider == REAL_SCOPED_PROVIDER_TAG
        && shape.result == ShapeResult::Scalar(BAD_DESCRIPTOR_RESULT)
        && shape.post_error == BAD_DESCRIPTOR_ERROR
        && shape.inputs.as_slice()
            == [ShapeLogicalInput {
                ordinal: 0,
                kind: DESCRIPTOR_HANDLE_KIND_TAG,
                resolution: ShapeLogicalInputResolution::Unknown,
            }]
        && shape.scalars.is_empty()
        && shape.byte_operands.is_empty()
        && shape.path_like_operands.is_empty()
        && shape.rooted_paths.is_empty()
        && shape.returned_paths.is_empty()
        && shape.returned_path_count == 0
        && shape.observed_regions.is_empty()
        && shape.metadata.is_empty()
        && shape.mutable_byte_resolutions.is_empty()
        && shape.mutable_i64_resolutions.is_empty()
        && shape.mutable_bytes.is_empty()
        && shape.mutable_i64s.is_empty()
        && shape.authorized_paths.is_empty()
        && shape.output.is_none()
        && shape.retired.is_empty()
        && shape.refusal_count == 0
}

pub(super) fn validate_unknown_descriptor_close_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_close_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor close is internally inconsistent",
        ))
    }
}
