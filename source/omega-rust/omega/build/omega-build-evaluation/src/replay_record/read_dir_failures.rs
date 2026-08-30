//! Exact replay-record shape for `read_dir` on an unknown descriptor.

use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeMutableI64, ShapeScalar,
    handle_failures::unknown_descriptor_failure_fixed_shape_is_exact,
};

const READ_DIR_OPERATION_TAG: u16 = 23;

pub(super) fn unknown_descriptor_read_dir_failure_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    let [(2, ShapeScalar::U64(requested_count))] = shape.scalars.as_slice() else {
        return false;
    };
    let [(1, buffer_resolution)] = shape.mutable_byte_resolutions.as_slice() else {
        return false;
    };
    let [buffer] = shape.mutable_bytes.as_slice() else {
        return false;
    };
    let [(3, position_resolution)] = shape.mutable_i64_resolutions.as_slice() else {
        return false;
    };
    let [
        ShapeMutableI64 {
            ordinal: 3,
            pre: position_pre,
            post: position_post,
        },
    ] = shape.mutable_i64s.as_slice()
    else {
        return false;
    };

    shape.operation == READ_DIR_OPERATION_TAG
        && shape.byte_operands.is_empty()
        && buffer.ordinal == 1
        && usize::try_from(*requested_count).is_ok_and(|count| count <= buffer_resolution.len())
        && *buffer_resolution == buffer.pre
        && buffer.pre == buffer.post
        && position_resolution == position_pre
        && position_pre == position_post
        && unknown_descriptor_failure_fixed_shape_is_exact(shape)
}

pub(super) fn validate_unknown_descriptor_read_dir_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_read_dir_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor read_dir failure is internally inconsistent",
        ))
    }
}
