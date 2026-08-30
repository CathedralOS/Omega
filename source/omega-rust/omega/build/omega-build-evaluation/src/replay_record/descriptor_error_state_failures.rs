//! Exact operand-free bad-descriptor failure and immediate `errno` shapes.

use super::handle_failures::operand_free_unknown_descriptor_failure_shape_is_exact;
use super::{AttemptShape, BuildFilesystemReplayRecordError, ShapeResult};

const ERRNO_OPERATION_TAG: u16 = 50;
const REAL_SCOPED_PROVIDER_TAG: u8 = 2;
const BAD_DESCRIPTOR_ERROR: i32 = 9;

pub(super) fn descriptor_operation_with_errno_shapes_are_exact(
    shapes: &[AttemptShape<'_>],
) -> bool {
    matches!(shapes, [operation, errno]
        if operand_free_unknown_descriptor_failure_shape_is_exact(operation)
            && errno_after_bad_descriptor_shape_is_exact(errno))
}

pub(super) fn validate_descriptor_operation_with_errno_shapes(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if descriptor_operation_with_errno_shapes_are_exact(shapes) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay descriptor operation and errno sequence is internally inconsistent",
        ))
    }
}

fn errno_after_bad_descriptor_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.operation == ERRNO_OPERATION_TAG
        && shape.provider == REAL_SCOPED_PROVIDER_TAG
        && shape.result == ShapeResult::Scalar(i64::from(BAD_DESCRIPTOR_ERROR))
        && shape.post_error == BAD_DESCRIPTOR_ERROR
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
        && shape.inputs.is_empty()
        && shape.output.is_none()
        && shape.retired.is_empty()
        && shape.refusal_count == 0
}
