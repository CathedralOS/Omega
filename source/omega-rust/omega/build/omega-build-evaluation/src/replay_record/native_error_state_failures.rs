//! Exact ordered native-handle failure and last-error replay shapes.

use super::native_mutation_failures::unknown_native_handle_mutation_failure_shape_is_exact;
use super::{AttemptShape, BuildFilesystemReplayRecordError, ShapeResult};

const GET_LAST_ERROR_OPERATION_TAG: u16 = 35;
const REAL_SCOPED_PROVIDER_TAG: u8 = 2;
const INVALID_HANDLE_ERROR: i32 = 6;

pub(super) fn native_mutation_with_last_error_shapes_are_exact(
    shapes: &[AttemptShape<'_>],
) -> bool {
    matches!(shapes, [mutation, error_read]
        if unknown_native_handle_mutation_failure_shape_is_exact(mutation)
            && last_error_after_invalid_handle_shape_is_exact(error_read))
}

pub(super) fn validate_native_mutation_with_last_error_shapes(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if native_mutation_with_last_error_shapes_are_exact(shapes) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay native mutation and last-error sequence is internally inconsistent",
        ))
    }
}

fn last_error_after_invalid_handle_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.operation == GET_LAST_ERROR_OPERATION_TAG
        && shape.provider == REAL_SCOPED_PROVIDER_TAG
        && shape.result == ShapeResult::Scalar(i64::from(INVALID_HANDLE_ERROR))
        && shape.post_error == INVALID_HANDLE_ERROR
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
