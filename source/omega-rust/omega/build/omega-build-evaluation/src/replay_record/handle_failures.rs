//! Exact failed logical-handle operations retained by build replay records.

use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeLogicalInput, ShapeLogicalInputResolution,
    ShapeResult, ShapeScalar,
};

const REAL_SCOPED_PROVIDER_TAG: u8 = 2;
const DESCRIPTOR_HANDLE_KIND_TAG: u8 = 0;
const NATIVE_HANDLE_KIND_TAG: u8 = 1;
const BAD_DESCRIPTOR_RESULT: i64 = -1;
const BAD_DESCRIPTOR_ERROR: i32 = 9;

/// Operations whose only authored operand is a descriptor and whose unknown-
/// descriptor result is fixed independently of host state.
pub(super) fn operand_free_unknown_descriptor_operation(operation: u16) -> bool {
    matches!(operation, 8 | 43 | 44 | 45)
}

pub(super) fn operand_free_unknown_descriptor_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    operand_free_unknown_descriptor_operation(shape.operation)
        && shape.scalars.is_empty()
        && unknown_descriptor_failure_base_is_exact(shape)
}

pub(super) fn validate_operand_free_unknown_descriptor_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if operand_free_unknown_descriptor_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay operand-free unknown-descriptor failure is internally inconsistent",
        ))
    }
}

pub(super) fn unknown_descriptor_seek_failure_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.operation == 10
        && matches!(
            shape.scalars.as_slice(),
            [(1, ShapeScalar::I64(_)), (2, ShapeScalar::I32(_))]
        )
        && unknown_descriptor_failure_base_is_exact(shape)
}

pub(super) fn unknown_descriptor_get_osfhandle_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    shape.operation == 30
        && shape.scalars.is_empty()
        && shape.mutable_byte_resolutions.is_empty()
        && shape.mutable_bytes.is_empty()
        && shape.byte_operands.is_empty()
        && unknown_descriptor_failure_core_except_bytes_with_outcome_is_exact(shape, -2, 0)
}

pub(super) fn unknown_native_handle_close_failure_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.operation == 29
        && shape.scalars.is_empty()
        && shape.mutable_byte_resolutions.is_empty()
        && shape.mutable_bytes.is_empty()
        && shape.byte_operands.is_empty()
        && unknown_handle_failure_core_except_bytes_with_outcome_is_exact(
            shape,
            NATIVE_HANDLE_KIND_TAG,
            0,
            6,
        )
}

pub(super) fn unknown_descriptor_write_operation(operation: u16) -> bool {
    matches!(operation, 17 | 41 | 46 | 49)
}

pub(super) fn unknown_descriptor_write_operation_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    let scalars_are_exact = match (shape.operation, shape.scalars.as_slice()) {
        (17, [(1, ShapeScalar::U32(_))])
        | (41, [(1, ShapeScalar::I64(_))])
        | (46, [(1, ShapeScalar::I32(_))])
        | (49, [(1, ShapeScalar::I32(_)), (2, ShapeScalar::I32(_))]) => true,
        _ => false,
    };
    scalars_are_exact && unknown_descriptor_failure_base_is_exact(shape)
}

pub(super) fn unknown_descriptor_set_file_times_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    let [(resolution_ordinal, resolution)] = shape.mutable_byte_resolutions.as_slice() else {
        return false;
    };
    let [carrier] = shape.mutable_bytes.as_slice() else {
        return false;
    };
    shape.operation == 42
        && shape.scalars.is_empty()
        && *resolution_ordinal == 1
        && carrier.ordinal == 1
        && resolution.len() >= 32
        && *resolution == carrier.pre
        && carrier.pre == carrier.post
        && unknown_descriptor_failure_core_is_exact(shape)
}

pub(super) fn unknown_descriptor_read_file_metadata_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    let [(resolution_ordinal, resolution)] = shape.mutable_byte_resolutions.as_slice() else {
        return false;
    };
    let [carrier] = shape.mutable_bytes.as_slice() else {
        return false;
    };
    shape.operation == 39
        && shape.scalars.is_empty()
        && *resolution_ordinal == 1
        && carrier.ordinal == 1
        && resolution.len() >= psi_checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        && *resolution == carrier.pre
        && carrier.pre == carrier.post
        && unknown_descriptor_failure_core_is_exact(shape)
}

pub(super) fn unknown_descriptor_read_operation(operation: u16) -> bool {
    matches!(operation, 4 | 6)
}

pub(super) fn unknown_descriptor_read_failure_shape_is_exact(shape: &AttemptShape<'_>) -> bool {
    let count = match (shape.operation, shape.scalars.as_slice()) {
        (4, [(2, ShapeScalar::U64(count))])
        | (6, [(2, ShapeScalar::U64(count)), (3, ShapeScalar::I64(_))]) => *count,
        _ => return false,
    };
    let [(resolution_ordinal, resolution)] = shape.mutable_byte_resolutions.as_slice() else {
        return false;
    };
    let [carrier] = shape.mutable_bytes.as_slice() else {
        return false;
    };
    *resolution_ordinal == 1
        && carrier.ordinal == 1
        && usize::try_from(count).is_ok_and(|count| count <= resolution.len())
        && *resolution == carrier.pre
        && carrier.pre == carrier.post
        && unknown_descriptor_failure_core_is_exact(shape)
}

pub(super) fn unknown_descriptor_write_payload_operation(operation: u16) -> bool {
    matches!(operation, 5 | 7)
}

pub(super) fn unknown_descriptor_write_payload_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    let scalars_are_exact = match (shape.operation, shape.scalars.as_slice()) {
        (5, []) | (7, [(2, ShapeScalar::I64(_))]) => true,
        _ => false,
    };
    let [(payload_ordinal, _payload)] = shape.byte_operands.as_slice() else {
        return false;
    };
    scalars_are_exact
        && *payload_ordinal == 1
        && unknown_descriptor_failure_core_except_bytes_is_exact(shape)
}

fn unknown_descriptor_failure_base_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.mutable_byte_resolutions.is_empty()
        && shape.mutable_bytes.is_empty()
        && unknown_descriptor_failure_core_is_exact(shape)
}

fn unknown_descriptor_failure_core_is_exact(shape: &AttemptShape<'_>) -> bool {
    shape.byte_operands.is_empty() && unknown_descriptor_failure_core_except_bytes_is_exact(shape)
}

fn unknown_descriptor_failure_core_except_bytes_is_exact(shape: &AttemptShape<'_>) -> bool {
    unknown_descriptor_failure_core_except_bytes_with_outcome_is_exact(
        shape,
        BAD_DESCRIPTOR_RESULT,
        BAD_DESCRIPTOR_ERROR,
    )
}

fn unknown_descriptor_failure_core_except_bytes_with_outcome_is_exact(
    shape: &AttemptShape<'_>,
    result: i64,
    post_error: i32,
) -> bool {
    unknown_handle_failure_core_except_bytes_with_outcome_is_exact(
        shape,
        DESCRIPTOR_HANDLE_KIND_TAG,
        result,
        post_error,
    )
}

fn unknown_handle_failure_core_except_bytes_with_outcome_is_exact(
    shape: &AttemptShape<'_>,
    handle_kind: u8,
    result: i64,
    post_error: i32,
) -> bool {
    shape.provider == REAL_SCOPED_PROVIDER_TAG
        && shape.result == ShapeResult::Scalar(result)
        && shape.post_error == post_error
        && shape.inputs.as_slice()
            == [ShapeLogicalInput {
                ordinal: 0,
                kind: handle_kind,
                resolution: ShapeLogicalInputResolution::Unknown,
            }]
        && shape.path_like_operands.is_empty()
        && shape.rooted_paths.is_empty()
        && shape.returned_paths.is_empty()
        && shape.returned_path_count == 0
        && shape.observed_regions.is_empty()
        && shape.metadata.is_empty()
        && shape.mutable_i64_resolutions.is_empty()
        && shape.mutable_i64s.is_empty()
        && shape.authorized_paths.is_empty()
        && shape.output.is_none()
        && shape.retired.is_empty()
        && shape.refusal_count == 0
}

pub(super) fn validate_unknown_native_handle_close_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_native_handle_close_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-native-handle close failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_get_osfhandle_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_get_osfhandle_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor get_osfhandle failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_write_payload_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_write_payload_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor write payload failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_write_operation_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_write_operation_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor write operation failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_set_file_times_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_set_file_times_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor set_file_times failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_read_file_metadata_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_read_file_metadata_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor read_file_metadata failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_read_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_read_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor read failure is internally inconsistent",
        ))
    }
}

pub(super) fn validate_unknown_descriptor_seek_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_descriptor_seek_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-descriptor seek failure is internally inconsistent",
        ))
    }
}
