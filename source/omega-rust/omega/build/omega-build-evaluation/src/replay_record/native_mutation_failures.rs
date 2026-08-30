//! Exact failed native-handle mutations retained by build replay records.

use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeScalar,
    handle_failures::unknown_handle_failure_core_except_bytes_with_outcome_is_exact,
};

const NATIVE_HANDLE_KIND_TAG: u8 = 1;
const INVALID_HANDLE_RESULT: i64 = 0;
const INVALID_HANDLE_ERROR: i32 = 6;
const FILETIME_BYTES: usize = 8;
const OVERLAPPED_BYTES: usize = 32;

pub(super) enum UnknownNativeHandleMutationShape<'a> {
    SetFileTime {
        creation: i64,
        last_access: &'a [u8],
        last_write: &'a [u8],
    },
    LockFileEx {
        flags: u32,
        reserved: u32,
        length_low: u32,
        length_high: u32,
        overlapped: &'a [u8],
    },
    UnlockFile {
        offset_low: u32,
        offset_high: u32,
        length_low: u32,
        length_high: u32,
    },
}

pub(super) fn unknown_native_handle_mutation_shape<'a>(
    shape: &AttemptShape<'a>,
) -> Option<UnknownNativeHandleMutationShape<'a>> {
    if !unknown_handle_failure_core_except_bytes_with_outcome_is_exact(
        shape,
        NATIVE_HANDLE_KIND_TAG,
        INVALID_HANDLE_RESULT,
        INVALID_HANDLE_ERROR,
    ) {
        return None;
    }
    match shape.operation {
        32 => {
            let [(1, ShapeScalar::I64(creation))] = shape.scalars.as_slice() else {
                return None;
            };
            let [(2, last_access), (3, last_write)] = shape.byte_operands.as_slice() else {
                return None;
            };
            (last_access.len() >= FILETIME_BYTES
                && last_write.len() >= FILETIME_BYTES
                && shape.mutable_byte_resolutions.is_empty()
                && shape.mutable_bytes.is_empty())
            .then_some(UnknownNativeHandleMutationShape::SetFileTime {
                creation: *creation,
                last_access,
                last_write,
            })
        }
        33 => {
            let [
                (1, ShapeScalar::U32(flags)),
                (2, ShapeScalar::U32(reserved)),
                (3, ShapeScalar::U32(length_low)),
                (4, ShapeScalar::U32(length_high)),
            ] = shape.scalars.as_slice()
            else {
                return None;
            };
            let [(5, resolution)] = shape.mutable_byte_resolutions.as_slice() else {
                return None;
            };
            let [carrier] = shape.mutable_bytes.as_slice() else {
                return None;
            };
            (shape.byte_operands.is_empty()
                && carrier.ordinal == 5
                && resolution.len() >= OVERLAPPED_BYTES
                && *resolution == carrier.pre
                && carrier.pre == carrier.post)
                .then_some(UnknownNativeHandleMutationShape::LockFileEx {
                    flags: *flags,
                    reserved: *reserved,
                    length_low: *length_low,
                    length_high: *length_high,
                    overlapped: resolution,
                })
        }
        34 => {
            let [
                (1, ShapeScalar::U32(offset_low)),
                (2, ShapeScalar::U32(offset_high)),
                (3, ShapeScalar::U32(length_low)),
                (4, ShapeScalar::U32(length_high)),
            ] = shape.scalars.as_slice()
            else {
                return None;
            };
            (shape.byte_operands.is_empty()
                && shape.mutable_byte_resolutions.is_empty()
                && shape.mutable_bytes.is_empty())
            .then_some(UnknownNativeHandleMutationShape::UnlockFile {
                offset_low: *offset_low,
                offset_high: *offset_high,
                length_low: *length_low,
                length_high: *length_high,
            })
        }
        _ => None,
    }
}

pub(super) fn unknown_native_handle_mutation_failure_shape_is_exact(
    shape: &AttemptShape<'_>,
) -> bool {
    unknown_native_handle_mutation_shape(shape).is_some()
}

pub(super) fn validate_unknown_native_handle_mutation_failure_shape(
    shape: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if unknown_native_handle_mutation_failure_shape_is_exact(shape) {
        Ok(())
    } else {
        Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-native-handle mutation failure is internally inconsistent",
        ))
    }
}
