//! Typed rehydration for one exact unknown-handle failure shape.

use super::handle_failures::{
    operand_free_unknown_descriptor_failure_shape_is_exact,
    operand_free_unknown_descriptor_operation,
    unknown_descriptor_get_osfhandle_failure_shape_is_exact,
    unknown_descriptor_open_at_failure_shape_is_exact,
    unknown_descriptor_read_failure_shape_is_exact,
    unknown_descriptor_read_file_metadata_failure_shape_is_exact,
    unknown_descriptor_read_operation, unknown_descriptor_seek_failure_shape_is_exact,
    unknown_descriptor_set_file_times_failure_shape_is_exact,
    unknown_descriptor_unlink_at_failure_shape_is_exact, unknown_descriptor_write_operation,
    unknown_descriptor_write_operation_failure_shape_is_exact,
    unknown_descriptor_write_payload_failure_shape_is_exact,
    unknown_descriptor_write_payload_operation, unknown_native_handle_close_failure_shape_is_exact,
    unknown_native_handle_final_path_failure_shape_is_exact,
};
use super::native_mutation_failures::unknown_native_handle_mutation_failure_shape_is_exact;
use super::read_dir_failures::unknown_descriptor_read_dir_failure_shape_is_exact;
use super::{
    AttemptShape, BuildFilesystemReplayRecordError, ShapeScalar, clone_bytes,
    rehydrate_operand_free_unknown_descriptor_kind, rehydrate_unknown_native_handle_mutation_kind,
};

pub(super) fn exact_single_failure_shape_is_supported(shape: &AttemptShape<'_>) -> bool {
    operand_free_unknown_descriptor_failure_shape_is_exact(shape)
        || unknown_descriptor_seek_failure_shape_is_exact(shape)
        || unknown_descriptor_open_at_failure_shape_is_exact(shape)
        || unknown_descriptor_unlink_at_failure_shape_is_exact(shape)
        || unknown_descriptor_read_dir_failure_shape_is_exact(shape)
        || unknown_descriptor_write_operation_failure_shape_is_exact(shape)
        || unknown_descriptor_set_file_times_failure_shape_is_exact(shape)
        || unknown_descriptor_read_failure_shape_is_exact(shape)
        || unknown_descriptor_write_payload_failure_shape_is_exact(shape)
        || unknown_descriptor_read_file_metadata_failure_shape_is_exact(shape)
        || unknown_descriptor_get_osfhandle_failure_shape_is_exact(shape)
        || unknown_native_handle_close_failure_shape_is_exact(shape)
        || unknown_native_handle_final_path_failure_shape_is_exact(shape)
        || unknown_native_handle_mutation_failure_shape_is_exact(shape)
}

pub(super) fn rehydrate_exact_single_failure_shape(
    source_input: Option<checked_interpreter::FilesystemSourceInputReplayRecord>,
    shape: &AttemptShape<'_>,
) -> Result<checked_interpreter::FilesystemReplay, BuildFilesystemReplayRecordError> {
    if !exact_single_failure_shape_is_supported(shape) {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay exact unknown-handle failure shape is unsupported",
        ));
    }

    if operand_free_unknown_descriptor_operation(shape.operation) {
        let kind = rehydrate_operand_free_unknown_descriptor_kind(shape.operation)?;
        let record =
            checked_interpreter::FilesystemInputUnknownDescriptorOperationReplayRecord::new(
                source_input,
                kind,
            );
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_operation_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay operand-free unknown-descriptor failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 10 {
        let [(1, ShapeScalar::I64(offset)), (2, ShapeScalar::I32(whence))] =
            shape.scalars.as_slice()
        else {
            unreachable!("validated unknown-descriptor seek retains exact scalar operands")
        };
        let record = checked_interpreter::FilesystemInputUnknownDescriptorSeekReplayRecord::new(
            source_input,
            *offset,
            *whence,
        );
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_seek_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor seek failure could not be rehydrated",
            )
        });
    }

    if matches!(shape.operation, 14 | 15) {
        let [(1, relative_component)] = shape.byte_operands.as_slice() else {
            unreachable!("validated unknown-descriptor at operation retains one exact component")
        };
        let [(2, ShapeScalar::I32(flags))] = shape.scalars.as_slice() else {
            unreachable!("validated unknown-descriptor at operation retains exact flags")
        };
        if shape.operation == 14 {
            let record =
                checked_interpreter::FilesystemInputUnknownDescriptorOpenAtReplayRecord::new(
                    source_input,
                    clone_bytes(relative_component)?,
                    *flags,
                )
                .map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay unknown-descriptor open_at record is inconsistent",
                    )
                })?;
            return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_open_at_record(
                record,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay unknown-descriptor open_at failure could not be rehydrated",
                )
            });
        }
        let record =
            checked_interpreter::FilesystemInputUnknownDescriptorUnlinkAtReplayRecord::new(
                source_input,
                clone_bytes(relative_component)?,
                *flags,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay unknown-descriptor unlink_at record is inconsistent",
                )
            })?;
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_unlink_at_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor unlink_at failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 23 {
        let [(2, ShapeScalar::U64(requested_count))] = shape.scalars.as_slice() else {
            unreachable!("validated unknown-descriptor read_dir retains exact requested count")
        };
        let [(1, buffer)] = shape.mutable_byte_resolutions.as_slice() else {
            unreachable!("validated unknown-descriptor read_dir retains exact buffer")
        };
        let [(3, position)] = shape.mutable_i64_resolutions.as_slice() else {
            unreachable!("validated unknown-descriptor read_dir retains exact position")
        };
        let record = checked_interpreter::FilesystemInputUnknownDescriptorReadDirReplayRecord::new(
            source_input,
            *requested_count,
            clone_bytes(buffer)?,
            *position,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor read_dir record is inconsistent",
            )
        })?;
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_dir_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor read_dir failure could not be rehydrated",
            )
        });
    }

    if unknown_descriptor_write_operation(shape.operation) {
        use checked_interpreter::FilesystemInputUnknownDescriptorWriteOperationReplayKind as Kind;
        let kind = match (shape.operation, shape.scalars.as_slice()) {
            (17, [(1, ShapeScalar::U32(mode))]) => Kind::SetFilePermissions { mode: *mode },
            (41, [(1, ShapeScalar::I64(length))]) => Kind::SetLength { length: *length },
            (46, [(1, ShapeScalar::I32(operation))]) => Kind::LockFile {
                operation: *operation,
            },
            (49, [(1, ShapeScalar::I32(uid)), (2, ShapeScalar::I32(gid))]) => {
                Kind::ChangeFileOwner {
                    uid: *uid,
                    gid: *gid,
                }
            }
            _ => unreachable!(
                "validated unknown-descriptor write operation retains exact scalar operands"
            ),
        };
        let record =
            checked_interpreter::FilesystemInputUnknownDescriptorWriteOperationReplayRecord::new(
                source_input,
                kind,
            );
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_operation_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor write operation failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 42 {
        let [(1, times)] = shape.mutable_byte_resolutions.as_slice() else {
            unreachable!("validated unknown-descriptor set_file_times retains one exact carrier")
        };
        let record =
            checked_interpreter::FilesystemInputUnknownDescriptorSetFileTimesReplayRecord::new(
                source_input,
                clone_bytes(times)?,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay unknown-descriptor set_file_times carrier could not be rehydrated",
                )
            })?;
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_set_file_times_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor set_file_times failure could not be rehydrated",
            )
        });
    }

    if unknown_descriptor_read_operation(shape.operation) {
        use checked_interpreter::FilesystemInputUnknownDescriptorReadReplayKind as Kind;
        let kind = match (shape.operation, shape.scalars.as_slice()) {
            (4, [(2, ShapeScalar::U64(count))]) => Kind::Sequential { count: *count },
            (6, [(2, ShapeScalar::U64(count)), (3, ShapeScalar::I64(offset))]) => {
                Kind::Positioned {
                    count: *count,
                    offset: *offset,
                }
            }
            _ => unreachable!("validated unknown-descriptor read retains exact scalar operands"),
        };
        let [(1, buffer)] = shape.mutable_byte_resolutions.as_slice() else {
            unreachable!("validated unknown-descriptor read retains one exact carrier")
        };
        let record = checked_interpreter::FilesystemInputUnknownDescriptorReadReplayRecord::new(
            source_input,
            kind,
            clone_bytes(buffer)?,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor read carrier could not be rehydrated",
            )
        })?;
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor read failure could not be rehydrated",
            )
        });
    }

    if unknown_descriptor_write_payload_operation(shape.operation) {
        use checked_interpreter::FilesystemInputUnknownDescriptorWriteReplayKind as Kind;
        let kind = match (shape.operation, shape.scalars.as_slice()) {
            (5, []) => Kind::Sequential,
            (7, [(2, ShapeScalar::I64(offset))]) => Kind::Positioned { offset: *offset },
            _ => unreachable!("validated unknown-descriptor write retains exact scalar operands"),
        };
        let [(1, payload)] = shape.byte_operands.as_slice() else {
            unreachable!("validated unknown-descriptor write retains one exact payload")
        };
        let record = checked_interpreter::FilesystemInputUnknownDescriptorWriteReplayRecord::new(
            source_input,
            kind,
            clone_bytes(payload)?,
        );
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_write_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor write failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 39 {
        let [(1, buffer)] = shape.mutable_byte_resolutions.as_slice() else {
            unreachable!(
                "validated unknown-descriptor read_file_metadata retains one exact carrier"
            )
        };
        let record =
            checked_interpreter::FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord::new(
                source_input,
                clone_bytes(buffer)?,
            );
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_read_file_metadata_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor read_file_metadata failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 30 {
        let record =
            checked_interpreter::FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord::new(
                source_input,
            );
        return checked_interpreter::FilesystemReplay::from_input_unknown_descriptor_get_osfhandle_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-descriptor get_osfhandle failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 29 {
        let record =
            checked_interpreter::FilesystemInputUnknownNativeHandleCloseHandleReplayRecord::new(
                source_input,
            );
        return checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_close_handle_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-native-handle close failure could not be rehydrated",
            )
        });
    }

    if shape.operation == 31 {
        let [(1, buffer)] = shape.mutable_byte_resolutions.as_slice() else {
            unreachable!("validated unknown-native-handle final path retains one exact carrier")
        };
        let [
            (2, ShapeScalar::U64(capacity)),
            (3, ShapeScalar::U32(flags)),
        ] = shape.scalars.as_slice()
        else {
            unreachable!("validated unknown-native-handle final path retains exact scalars")
        };
        let record = checked_interpreter::FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord::new(
            source_input,
            clone_bytes(buffer)?,
            *capacity,
            *flags,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-native-handle final path record is inconsistent",
            )
        })?;
        return checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_final_path_name_by_handle_record(
            record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-native-handle final path failure could not be rehydrated",
            )
        });
    }

    let kind = rehydrate_unknown_native_handle_mutation_kind(shape)?;
    let record = checked_interpreter::FilesystemInputUnknownNativeHandleMutationReplayRecord::new(
        source_input,
        kind,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-native-handle mutation record is inconsistent",
        )
    })?;
    checked_interpreter::FilesystemReplay::from_input_unknown_native_handle_mutation_record(record)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay unknown-native-handle mutation failure could not be rehydrated",
            )
        })
}
