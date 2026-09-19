//! Rehydrating a review-only record into exact replay shapes: source
//! directories, output files, metadata, reads and native query chains.

use crate::evidence::replay_record::attempt_codec::{
    AttemptShape, Decoder, ShapeResult, ShapeScalar, decode_attempt, decode_replay_activation,
};
use crate::evidence::replay_record::descriptor_error_state_failures::unknown_descriptor_failure_with_errno_shapes_are_exact;
use crate::evidence::replay_record::exact_failure_rehydration::{
    exact_single_failure_shape_is_supported, rehydrate_exact_single_failure_shape,
};
use crate::evidence::replay_record::hard_links::rehydrate_output_hard_link_shape;
use crate::evidence::replay_record::native_error_state_failures::unknown_native_handle_failure_with_last_error_shapes_are_exact;
use crate::evidence::replay_record::native_mutation_failures::{
    UnknownNativeHandleMutationShape, unknown_native_handle_mutation_shape,
};
use crate::evidence::replay_record::read_links::rehydrate_source_read_link_shape;
use crate::evidence::replay_record::record::{MAGIC, VERSION, clone_bytes};
use crate::evidence::replay_record::shape_validation::{
    OutputEntryAttempts, output_tree_membership, validate_first_rung,
    validate_included_source_shapes, validate_source_write_refusal_shape,
};
use crate::evidence::replay_record::symlinks::rehydrate_output_symlink_shape;
use crate::evidence::replay_record::{
    BuildFilesystemReplayRecordError, BuildFilesystemReplayRecordLimits,
    ReviewOnlyBuildFilesystemReplayRecord,
};
use crate::{BuildCanonicalSourceMetadataIdentity, BuildReplayActivation};

pub(crate) fn rehydrate_unknown_native_handle_mutation_kind(
    shape: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemInputUnknownNativeHandleMutationReplayKind,
    BuildFilesystemReplayRecordError,
> {
    use checked_interpreter::FilesystemInputUnknownNativeHandleMutationReplayKind as Kind;
    match unknown_native_handle_mutation_shape(shape) {
        Some(UnknownNativeHandleMutationShape::SetFileTime {
            creation,
            last_access,
            last_write,
        }) => Ok(Kind::SetFileTime {
            creation,
            last_access: clone_bytes(last_access)?,
            last_write: clone_bytes(last_write)?,
        }),
        Some(UnknownNativeHandleMutationShape::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped,
        }) => Ok(Kind::LockFileEx {
            flags,
            reserved,
            length_low,
            length_high,
            overlapped: clone_bytes(overlapped)?,
        }),
        Some(UnknownNativeHandleMutationShape::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        }) => Ok(Kind::UnlockFile {
            offset_low,
            offset_high,
            length_low,
            length_high,
        }),
        None => Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay unknown-native-handle mutation inputs are inconsistent",
        )),
    }
}

pub(crate) fn rehydrate_operand_free_unknown_descriptor_kind(
    operation: u16,
) -> Result<
    checked_interpreter::FilesystemInputUnknownDescriptorOperationReplayKind,
    BuildFilesystemReplayRecordError,
> {
    use checked_interpreter::FilesystemInputUnknownDescriptorOperationReplayKind as Kind;
    match operation {
        8 => Ok(Kind::Close),
        43 => Ok(Kind::Sync),
        44 => Ok(Kind::SyncData),
        45 => Ok(Kind::Duplicate),
        _ => Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay operand-free descriptor operation is inconsistent",
        )),
    }
}

pub fn rehydrate_review_only_build_filesystem_replay_record(
    record: &ReviewOnlyBuildFilesystemReplayRecord,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<checked_interpreter::FilesystemReplay, BuildFilesystemReplayRecordError> {
    let decoded = decode_shapes(record.canonical_bytes(), limits)?;
    let included_sources = decoded.included_sources;
    let shapes = decoded.shapes;
    let operation_suffix_start = shapes
        .iter()
        .position(|shape| matches!(shape.operation, 1 | 9 | 11 | 12 | 19 | 20 | 27))
        .or_else(|| {
            shapes.len().checked_sub(2).filter(|suffix_start| {
                unknown_native_handle_failure_with_last_error_shapes_are_exact(
                    &shapes[*suffix_start..],
                )
            })
        })
        .or_else(|| {
            shapes.len().checked_sub(2).filter(|suffix_start| {
                unknown_descriptor_failure_with_errno_shapes_are_exact(&shapes[*suffix_start..])
            })
        })
        .or_else(|| {
            shapes
                .last()
                .filter(|shape| exact_single_failure_shape_is_supported(shape))
                .map(|_| shapes.len() - 1)
        })
        .unwrap_or(shapes.len());
    let mut events = Vec::new();
    let mut cursor = 0;
    while cursor < operation_suffix_start {
        if shapes[cursor].operation == 21 {
            events.push(
                checked_interpreter::FilesystemSourceInputReplayEventRecord::ReadLink(
                    rehydrate_source_read_link_shape(&shapes[cursor])?,
                ),
            );
            cursor += 1;
            continue;
        }
        if matches!(shapes[cursor].operation, 38 | 40) {
            events.push(
                checked_interpreter::FilesystemSourceInputReplayEventRecord::PathMetadata(
                    rehydrate_path_metadata_shape(&shapes[cursor])?,
                ),
            );
            cursor += 1;
            continue;
        }
        if shapes[cursor].operation == 28 {
            let open = &shapes[cursor];
            cursor += 1;
            let operations_start = cursor;
            while matches!(shapes[cursor].operation, 31 | 35) {
                cursor += 1;
            }
            let close = &shapes[cursor];
            cursor += 1;
            events.push(
                checked_interpreter::FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(
                    rehydrate_native_query_chain_shape(
                        open,
                        &shapes[operations_start..cursor - 1],
                        close,
                    )?,
                ),
            );
            continue;
        }
        let open = &shapes[cursor];
        cursor += 1;
        if shapes[cursor].operation == 39 {
            let metadata = &shapes[cursor];
            let close = &shapes[cursor + 1];
            cursor += 2;
            events.push(
                checked_interpreter::FilesystemSourceInputReplayEventRecord::DescriptorMetadata(
                    rehydrate_descriptor_metadata_shape(open, metadata, close)?,
                ),
            );
            continue;
        }
        if shapes[cursor].operation == 23 {
            let reads_start = cursor;
            while shapes[cursor].operation == 23 {
                cursor += 1;
            }
            let close = &shapes[cursor];
            events.push(
                checked_interpreter::FilesystemSourceInputReplayEventRecord::DirectoryReadChain(
                    rehydrate_source_directory_shape(open, &shapes[reads_start..cursor], close)?,
                ),
            );
            cursor += 1;
            continue;
        }
        let reads_start = cursor;
        while matches!(shapes.get(cursor), Some(read) if matches!(read.operation, 4 | 6)) {
            cursor += 1;
        }
        let close = &shapes[cursor];
        let read_shapes = &shapes[reads_start..cursor];
        cursor += 1;

        let ShapeResult::Handle(logical_handle_identity) = open.result else {
            unreachable!("validated bounded replay open returns a handle")
        };
        let [source_path] = open.rooted_paths.as_slice() else {
            unreachable!("validated bounded replay open has one rooted path")
        };
        let mut reads = Vec::new();
        reads.try_reserve_exact(read_shapes.len()).map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay read allocation failed")
        })?;
        for read in read_shapes {
            reads.push(rehydrate_read_shape(read)?);
        }
        events.push(
            checked_interpreter::FilesystemSourceInputReplayEventRecord::ReadChain(
                checked_interpreter::FilesystemSourceReadChainReplayRecord::new(
                    crate::BUILD_SOURCE_ROOT_IDENTITY,
                    clone_bytes(source_path.bytes)?,
                    logical_handle_identity,
                    open.post_error,
                    reads,
                    close.post_error,
                )
                .map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay chain could not be rehydrated",
                    )
                })?,
            ),
        );
    }
    let typed_source_record = if events.is_empty() {
        None
    } else {
        Some(
            checked_interpreter::FilesystemSourceInputReplayRecord::new(events).map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay source inputs could not be rehydrated",
                )
            })?,
        )
    };
    if operation_suffix_start == shapes.len() {
        let typed_source_record = typed_source_record.ok_or_else(|| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay source-only record has no Source events",
            )
        })?;
        return checked_interpreter::FilesystemReplay::from_source_input_record(
            typed_source_record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay source inputs exceed retained replay policy",
            )
        });
    }
    if operation_suffix_start == 0 && shapes.len() == 1 && matches!(shapes[0].operation, 1 | 9) {
        validate_source_write_refusal_shape(&shapes[0])?;
        let [rooted] = shapes[0].rooted_paths.as_slice() else {
            unreachable!("validated refused Source write has one rooted path")
        };
        let record = checked_interpreter::FilesystemSourceWriteRefusalReplayRecord::new(
            if shapes[0].operation == 1 {
                checked_interpreter::FilesystemSourceWriteRefusalReplayKind::Create
            } else {
                checked_interpreter::FilesystemSourceWriteRefusalReplayKind::RemoveFile
            },
            crate::BUILD_SOURCE_ROOT_IDENTITY,
            clone_bytes(rooted.bytes)?,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay refused Source write could not be rehydrated",
            )
        })?;
        return checked_interpreter::FilesystemReplay::from_source_write_refusal_record(record)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay refused Source write exceeds retained replay policy",
                )
            });
    }
    if shapes.len() - operation_suffix_start == 2
        && unknown_descriptor_failure_with_errno_shapes_are_exact(&shapes[operation_suffix_start..])
    {
        let replay = rehydrate_exact_single_failure_shape(
            typed_source_record,
            &shapes[operation_suffix_start],
        )?;
        return replay
            .with_immediate_errno_after_unknown_descriptor_failure()
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay descriptor operation and errno sequence could not be rehydrated",
                )
            });
    }
    if shapes.len() - operation_suffix_start == 1
        && exact_single_failure_shape_is_supported(&shapes[operation_suffix_start])
    {
        return rehydrate_exact_single_failure_shape(
            typed_source_record,
            &shapes[operation_suffix_start],
        );
    }
    if shapes.len() - operation_suffix_start == 2
        && unknown_native_handle_failure_with_last_error_shapes_are_exact(
            &shapes[operation_suffix_start..],
        )
    {
        let replay = rehydrate_exact_single_failure_shape(
            typed_source_record,
            &shapes[operation_suffix_start],
        )?;
        return replay
            .with_immediate_last_error_after_unknown_native_handle_failure()
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay native-handle failure and last-error sequence could not be rehydrated",
                )
            });
    }
    if shapes[operation_suffix_start..]
        .iter()
        .all(|shape| matches!(shape.operation, 9 | 12))
    {
        let mut absent_removes = Vec::new();
        absent_removes
            .try_reserve_exact(shapes.len() - operation_suffix_start)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay absent-remove allocation failed",
                )
            })?;
        for shape in &shapes[operation_suffix_start..] {
            let [rooted] = shape.rooted_paths.as_slice() else {
                unreachable!("validated absent Output remove has one rooted path")
            };
            let kind = match shape.operation {
                9 => checked_interpreter::FilesystemOutputAbsentRemoveKind::File,
                12 => checked_interpreter::FilesystemOutputAbsentRemoveKind::Directory,
                _ => unreachable!("validated absent Output remove has an exact operation"),
            };
            absent_removes.push(
                checked_interpreter::FilesystemOutputAbsentRemoveReplayRecord::new(
                    kind,
                    crate::BUILD_OUTPUT_ROOT_IDENTITY,
                    clone_bytes(rooted.bytes)?,
                )
                .map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay absent Output remove could not be rehydrated",
                    )
                })?,
            );
        }
        let typed_record =
            checked_interpreter::FilesystemInputOutputAbsentRemovesReplayRecord::new(
                typed_source_record,
                absent_removes,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay absent Output removes could not be rehydrated",
                )
            })?;
        return checked_interpreter::FilesystemReplay::from_input_output_absent_removes_record(
            typed_record,
        )
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output removes exceed retained replay policy",
            )
        });
    }
    let output_stream = output_tree_membership(&shapes, operation_suffix_start)?;
    let mut ordered_attempts = Vec::new();
    ordered_attempts
        .try_reserve_exact(shapes.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay Output-tree allocation failed")
        })?;
    ordered_attempts.resize_with(shapes.len(), || None);
    if let Some(source) = typed_source_record {
        let source = checked_interpreter::FilesystemReplay::from_source_input_record(source)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new("filesystem replay Source prefix is invalid")
            })?;
        if source.attempts().len() != operation_suffix_start {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay Source prefix coverage differs",
            ));
        }
        for (slot, attempt) in ordered_attempts.iter_mut().zip(source.attempts()) {
            *slot = Some(attempt.clone());
        }
    } else if operation_suffix_start != 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output record has a malformed non-Source prefix",
        ));
    }
    for range in output_stream {
        let entry = match &range {
            OutputEntryAttempts::Directory(index) => {
                let [rooted] = shapes[*index].rooted_paths.as_slice() else {
                    unreachable!("validated Output directory has one rooted path")
                };
                checked_interpreter::FilesystemOutputTreeEntryReplayRecord::Directory(
                    checked_interpreter::FilesystemOutputDirectoryReplayRecord::new(
                        crate::BUILD_OUTPUT_ROOT_IDENTITY,
                        clone_bytes(rooted.bytes)?,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output directory could not be rehydrated",
                        )
                    })?,
                )
            }
            OutputEntryAttempts::File { attempts } => {
                let chain = attempts
                    .iter()
                    .map(|position| &shapes[*position])
                    .collect::<Vec<_>>();
                checked_interpreter::FilesystemOutputTreeEntryReplayRecord::File(
                    rehydrate_output_file_shape(&chain)?,
                )
            }
            OutputEntryAttempts::HardLink(index) => {
                checked_interpreter::FilesystemOutputTreeEntryReplayRecord::HardLink(
                    rehydrate_output_hard_link_shape(&shapes[*index])?,
                )
            }
            OutputEntryAttempts::Symlink(index) => {
                checked_interpreter::FilesystemOutputTreeEntryReplayRecord::Symlink(
                    rehydrate_output_symlink_shape(&shapes[*index])?,
                )
            }
        };
        let regenerated = entry.into_attempts();
        if regenerated.len() != range.attempt_indices().len() {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay Output event coverage differs",
            ));
        }
        for (position, attempt) in range.attempt_indices().iter().zip(regenerated) {
            if ordered_attempts[*position].replace(attempt).is_some() {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Output event is owned twice",
                ));
            }
        }
    }
    let mut expected_included_sources = Vec::new();
    expected_included_sources
        .try_reserve_exact(included_sources.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source allocation failed",
            )
        })?;
    for included in included_sources {
        expected_included_sources.push(
            checked_interpreter::BuildIncludedSource::from_coordinate(
                crate::BUILD_OUTPUT_ROOT_IDENTITY,
                clone_bytes(included.relative_path)?,
                usize::try_from(included.filesystem_attempt_ordinal).map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "filesystem replay included-source ordinal exceeds this compiler host",
                    )
                })?,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay generated-source handoff could not be rehydrated",
                )
            })?,
        );
    }
    let attempts = ordered_attempts
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("filesystem replay event coverage is incomplete")
        })?;
    checked_interpreter::FilesystemReplay::from_input_output_attempts(
        attempts,
        expected_included_sources,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay input/Output-tree record exceeds retained replay policy",
        )
    })
}

fn rehydrate_source_directory_shape(
    open: &AttemptShape<'_>,
    reads: &[AttemptShape<'_>],
    close: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemSourceDirectoryReadChainReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let ShapeResult::Handle(identity) = open.result else {
        unreachable!("validated directory replay open returns one handle")
    };
    let [source_path] = open.rooted_paths.as_slice() else {
        unreachable!("validated directory replay open has one rooted path")
    };
    let mut records = Vec::new();
    records.try_reserve_exact(reads.len()).map_err(|_| {
        BuildFilesystemReplayRecordError::new("filesystem directory replay allocation failed")
    })?;
    for read in reads {
        let ShapeResult::Scalar(result) = read.result else {
            unreachable!("validated directory replay read returns one scalar")
        };
        let [(2, ShapeScalar::U64(requested))] = read.scalars.as_slice() else {
            unreachable!("validated directory replay read has one count")
        };
        let [(1, resolution)] = read.mutable_byte_resolutions.as_slice() else {
            unreachable!("validated directory replay read has one byte resolution")
        };
        let [carrier] = read.mutable_bytes.as_slice() else {
            unreachable!("validated directory replay read has one byte carrier")
        };
        let [(3, position_resolution)] = read.mutable_i64_resolutions.as_slice() else {
            unreachable!("validated directory replay read has one cursor resolution")
        };
        let [position] = read.mutable_i64s.as_slice() else {
            unreachable!("validated directory replay read has one cursor carrier")
        };
        records.push(
            checked_interpreter::FilesystemSourceDirectoryReadReplayRecord::new(
                *requested,
                result,
                read.post_error,
                clone_bytes(resolution)?,
                clone_bytes(carrier.pre)?,
                clone_bytes(carrier.post)?,
                *position_resolution,
                position.pre,
                position.post,
            )
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem directory replay read could not be rehydrated",
                )
            })?,
        );
    }
    checked_interpreter::FilesystemSourceDirectoryReadChainReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(source_path.bytes)?,
        identity,
        open.post_error,
        records,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem directory replay chain could not be rehydrated",
        )
    })
}

fn rehydrate_output_file_shape(
    chain: &[&AttemptShape<'_>],
) -> Result<checked_interpreter::FilesystemOutputFileReplayRecord, BuildFilesystemReplayRecordError>
{
    let create = &chain[0];
    let close = chain.last().expect("validated Output file has a close");
    let operations = &chain[1..chain.len() - 1];
    let Some(output) = create.output else {
        unreachable!("validated receipted output create has a descriptor")
    };
    let [rooted] = create.rooted_paths.as_slice() else {
        unreachable!("validated receipted output create has one rooted path")
    };
    let mut operation_records = Vec::new();
    operation_records
        .try_reserve_exact(operations.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay output-operation allocation failed",
            )
        })?;
    let mut operation_cursor = 0;
    while operation_cursor < operations.len() {
        let operation = &operations[operation_cursor];
        let record = match operation.operation {
            45 => {
                let Some(duplicate) = operation.output else {
                    unreachable!("validated Output duplicate has one fresh identity")
                };
                operation_cursor += 2;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(
                    checked_interpreter::FilesystemOutputDuplicateReplayRecord::new(
                        duplicate.identity,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output duplicate could not be rehydrated",
                        )
                    })?,
                )
            }
            46 => {
                let release = &operations[operation_cursor + 1];
                let [(1, ShapeScalar::I32(acquire_operation))] = operation.scalars.as_slice()
                else {
                    unreachable!("validated Output lock acquire has one i32 scalar")
                };
                let [(1, ShapeScalar::I32(release_operation))] = release.scalars.as_slice() else {
                    unreachable!("validated Output lock release has one i32 scalar")
                };
                let ShapeResult::Scalar(acquire_result) = operation.result else {
                    unreachable!("validated Output lock acquire has one scalar result")
                };
                let ShapeResult::Scalar(release_result) = release.result else {
                    unreachable!("validated Output lock release has one scalar result")
                };
                operation_cursor += 2;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::LockAndUnlock(
                    checked_interpreter::FilesystemOutputLockReplayRecord::new(
                        *acquire_operation,
                        acquire_result,
                        operation.post_error,
                        *release_operation,
                        release_result,
                        release.post_error,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output lock could not be rehydrated",
                        )
                    })?,
                )
            }
            10 => {
                let [(1, ShapeScalar::I64(offset)), (2, ShapeScalar::I32(whence))] =
                    operation.scalars.as_slice()
                else {
                    unreachable!("validated Output seek has exact offset and whence")
                };
                let ShapeResult::Scalar(result) = operation.result else {
                    unreachable!("validated Output seek returns a scalar")
                };
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::Seek {
                    offset: *offset,
                    whence: *whence,
                    result,
                }
            }
            41 => {
                let [(1, ShapeScalar::I64(length))] = operation.scalars.as_slice() else {
                    unreachable!("validated Output set_len has one i64 length")
                };
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::SetLength {
                    length: *length,
                }
            }
            17 => {
                let [(1, ShapeScalar::U32(mode))] = operation.scalars.as_slice() else {
                    unreachable!("validated Output set_file_permissions has one u32 mode")
                };
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::SetFilePermissions {
                    mode: *mode,
                }
            }
            42 => {
                let [(1, times)] = operation.mutable_byte_resolutions.as_slice() else {
                    unreachable!("validated Output set_file_times has one exact carrier")
                };
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::SetFileTimes {
                    times: clone_bytes(times)?,
                }
            }
            43 => {
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::Sync
            }
            44 => {
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::SyncData
            }
            49 => {
                let [(1, ShapeScalar::I32(uid)), (2, ShapeScalar::I32(gid))] =
                    operation.scalars.as_slice()
                else {
                    unreachable!("validated Output change_file_owner has exact uid and gid")
                };
                let ShapeResult::Scalar(result) = operation.result else {
                    unreachable!("validated Output change_file_owner returns a scalar")
                };
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                    checked_interpreter::FilesystemOutputChangeFileOwnerReplayRecord::new(
                        *uid,
                        *gid,
                        result,
                        operation.post_error,
                    )
                    .map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output change_file_owner could not be rehydrated",
                        )
                    })?,
                )
            }
            5 | 7 => {
                let [(_, payload)] = operation.byte_operands.as_slice() else {
                    unreachable!("validated receipted output write has one payload")
                };
                let ShapeResult::Scalar(write_result) = operation.result else {
                    unreachable!("validated receipted output write returns a scalar")
                };
                let write_record = if operation.operation == 5 {
                    checked_interpreter::FilesystemOutputWriteReplayRecord::new(
                        clone_bytes(payload)?,
                        write_result,
                        operation.post_error,
                    )
                } else {
                    let [(2, ShapeScalar::I64(offset))] = operation.scalars.as_slice() else {
                        unreachable!("validated positioned write has one i64 offset")
                    };
                    checked_interpreter::FilesystemOutputWriteReplayRecord::positioned(
                        *offset,
                        clone_bytes(payload)?,
                        write_result,
                        operation.post_error,
                    )
                };
                operation_cursor += 1;
                checked_interpreter::FilesystemOutputFileOperationReplayRecord::Write(
                    write_record.map_err(|_| {
                        BuildFilesystemReplayRecordError::new(
                            "filesystem replay output write could not be rehydrated",
                        )
                    })?,
                )
            }
            _ => unreachable!("validated Output file has an admitted operation"),
        };
        operation_records.push(record);
    }
    checked_interpreter::FilesystemOutputFileReplayRecord::with_operations(
        crate::BUILD_OUTPUT_ROOT_IDENTITY,
        clone_bytes(rooted.bytes)?,
        output.identity,
        create.post_error,
        operation_records,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Output file could not be rehydrated",
        )
    })
}

fn rehydrate_path_metadata_shape(
    shape: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemSourcePathMetadataReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let [rooted] = shape.rooted_paths.as_slice() else {
        unreachable!("validated source metadata has one rooted input")
    };
    let [authorized] = shape.authorized_paths.as_slice() else {
        unreachable!("validated source metadata has one authorized target")
    };
    let [metadata] = shape.metadata.as_slice() else {
        unreachable!("validated source metadata has one semantic row")
    };
    let [(1, mutable_resolution)] = shape.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated source metadata has one mutable resolution")
    };
    let [mutable] = shape.mutable_bytes.as_slice() else {
        unreachable!("validated source metadata has one mutable carrier")
    };
    let kind = match shape.operation {
        38 => checked_interpreter::FilesystemMetadataObservationKind::FollowedPath,
        40 => checked_interpreter::FilesystemMetadataObservationKind::UnfollowedFinalPath,
        _ => unreachable!("validated source metadata operation"),
    };
    let metadata = checked_interpreter::FilesystemMetadataObservation::from_replay(
        kind,
        metadata.device,
        metadata.mode,
        metadata.link_count,
        metadata.inode,
        metadata.user,
        metadata.group,
        metadata.referenced_device,
        metadata.access_time,
        metadata.modification_time,
        metadata.change_time,
        metadata.birth_time,
        metadata.size,
        metadata.blocks_512,
        metadata.preferred_block_size,
    );
    checked_interpreter::FilesystemSourcePathMetadataReplayRecord::new(
        kind,
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(rooted.bytes)?,
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(authorized.bytes)?,
        shape.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable.pre)?,
        clone_bytes(mutable.post)?,
        metadata,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay path metadata could not be rehydrated",
        )
    })
}

fn rehydrate_descriptor_metadata_shape(
    open: &AttemptShape<'_>,
    metadata_shape: &AttemptShape<'_>,
    close: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemSourceDescriptorMetadataReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let ShapeResult::Handle(logical_handle_identity) = open.result else {
        unreachable!("validated descriptor metadata open returns a handle")
    };
    let [source_path] = open.rooted_paths.as_slice() else {
        unreachable!("validated descriptor metadata open has one source path")
    };
    let [metadata] = metadata_shape.metadata.as_slice() else {
        unreachable!("validated descriptor metadata has one semantic row")
    };
    let [(1, mutable_resolution)] = metadata_shape.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated descriptor metadata has one mutable resolution")
    };
    let [mutable] = metadata_shape.mutable_bytes.as_slice() else {
        unreachable!("validated descriptor metadata has one mutable carrier")
    };
    let metadata = checked_interpreter::FilesystemMetadataObservation::from_replay(
        checked_interpreter::FilesystemMetadataObservationKind::OpenDescriptor,
        metadata.device,
        metadata.mode,
        metadata.link_count,
        metadata.inode,
        metadata.user,
        metadata.group,
        metadata.referenced_device,
        metadata.access_time,
        metadata.modification_time,
        metadata.change_time,
        metadata.birth_time,
        metadata.size,
        metadata.blocks_512,
        metadata.preferred_block_size,
    );
    checked_interpreter::FilesystemSourceDescriptorMetadataReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(source_path.bytes)?,
        logical_handle_identity,
        open.post_error,
        metadata_shape.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable.pre)?,
        clone_bytes(mutable.post)?,
        metadata,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay descriptor metadata could not be rehydrated",
        )
    })
}

fn rehydrate_read_shape(
    read: &AttemptShape<'_>,
) -> Result<checked_interpreter::FilesystemReplayReadRecord, BuildFilesystemReplayRecordError> {
    let ShapeResult::Scalar(read_result) = read.result else {
        unreachable!("validated bounded replay read returns a scalar")
    };
    let (read_kind, requested_count) = match read.scalars.as_slice() {
        [(2, ShapeScalar::U64(requested_count))] => (
            checked_interpreter::FilesystemReplayReadKind::Sequential,
            requested_count,
        ),
        [
            (2, ShapeScalar::U64(requested_count)),
            (3, ShapeScalar::I64(offset)),
        ] => (
            checked_interpreter::FilesystemReplayReadKind::Positioned { offset: *offset },
            requested_count,
        ),
        _ => unreachable!("validated bounded replay read has exact count and optional offset"),
    };
    let [(1, mutable_resolution)] = read.mutable_byte_resolutions.as_slice() else {
        unreachable!("validated bounded replay read has one mutable resolution")
    };
    let [mutable_carrier] = read.mutable_bytes.as_slice() else {
        unreachable!("validated bounded replay read has one mutable carrier")
    };
    checked_interpreter::FilesystemReplayReadRecord::new(
        read_kind,
        *requested_count,
        read_result,
        read.post_error,
        clone_bytes(mutable_resolution)?,
        clone_bytes(mutable_carrier.pre)?,
        clone_bytes(mutable_carrier.post)?,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new("filesystem replay read could not be rehydrated")
    })
}

fn rehydrate_native_query_chain_shape(
    open: &AttemptShape<'_>,
    operations: &[AttemptShape<'_>],
    close: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemSourceNativeHandleQueryChainReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    let ShapeResult::Handle(logical_handle_identity) = open.result else {
        unreachable!("validated native-handle query open returns a handle")
    };
    let [source_path] = open.rooted_paths.as_slice() else {
        unreachable!("validated native-handle query open has one rooted path")
    };
    let ShapeResult::Scalar(close_result) = close.result else {
        unreachable!("validated native-handle query close returns a scalar")
    };
    let mut records = Vec::new();
    records.try_reserve_exact(operations.len()).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle query allocation failed",
        )
    })?;
    for operation in operations {
        records.push(rehydrate_native_query_operation_shape(operation)?);
    }
    checked_interpreter::FilesystemSourceNativeHandleQueryChainReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        clone_bytes(source_path.bytes)?,
        logical_handle_identity,
        open.post_error,
        records,
        close_result,
        close.post_error,
    )
    .map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle query chain could not be rehydrated",
        )
    })
}

fn rehydrate_native_query_operation_shape(
    operation: &AttemptShape<'_>,
) -> Result<
    checked_interpreter::FilesystemNativeHandleQueryOperationReplayRecord,
    BuildFilesystemReplayRecordError,
> {
    use checked_interpreter::FilesystemNativeHandleQueryOperationReplayRecord as Operation;
    match operation.operation {
        31 => {
            let ShapeResult::Scalar(result) = operation.result else {
                unreachable!("validated native-handle final-path query returns a scalar")
            };
            let [
                (2, ShapeScalar::U64(capacity)),
                (3, ShapeScalar::U32(flags)),
            ] = operation.scalars.as_slice()
            else {
                unreachable!("validated native-handle final-path query retains exact scalars")
            };
            let [returned] = operation.returned_paths.as_slice() else {
                unreachable!("validated native-handle final-path query has one returned path")
            };
            let [(1, resolution)] = operation.mutable_byte_resolutions.as_slice() else {
                unreachable!("validated native-handle final-path query has one mutable resolution")
            };
            let [carrier] = operation.mutable_bytes.as_slice() else {
                unreachable!("validated native-handle final-path query has one mutable carrier")
            };
            checked_interpreter::FilesystemNativeHandleFinalPathQueryReplayRecord::new(
                *capacity,
                *flags,
                result,
                operation.post_error,
                clone_bytes(resolution)?,
                clone_bytes(carrier.pre)?,
                clone_bytes(carrier.post)?,
                clone_bytes(returned.bytes)?,
            )
            .map(Operation::FinalPathName)
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay native-handle final-path query could not be rehydrated",
                )
            })
        }
        35 => Ok(Operation::LastError(
            checked_interpreter::FilesystemNativeHandleErrorObservationReplayRecord::new(
                operation.post_error,
            ),
        )),
        _ => Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle query operation is unsupported",
        )),
    }
}

pub(crate) struct DecodedReplay<'a> {
    pub(crate) canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
    pub(crate) captured_source_inventory: Option<crate::BuildCapturedSourceInventory>,
    pub(crate) replay_activation: BuildReplayActivation,
    included_sources: Vec<ShapeIncludedSource<'a>>,
    shapes: Vec<AttemptShape<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShapeIncludedSource<'a> {
    pub(crate) relative_path: &'a [u8],
    pub(crate) filesystem_attempt_ordinal: u64,
}

pub(crate) fn decode_shapes(
    bytes: &[u8],
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<DecodedReplay<'_>, BuildFilesystemReplayRecordError> {
    if bytes.len() > limits.maximum_bytes {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record exceeds its byte ceiling",
        ));
    }
    let mut decoder = Decoder::new(bytes, limits);
    decoder.fixed(MAGIC)?;
    if decoder.u16()? != VERSION {
        return Err(BuildFilesystemReplayRecordError::new(
            "unsupported filesystem replay record version",
        ));
    }
    if decoder.u32()? != crate::BUILD_OBSERVATION_SCHEMA_VERSION
        || decoder.u32()? != checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "unsupported filesystem replay semantic schema",
        ));
    }
    let canonical_source_metadata_identity =
        decode_canonical_source_metadata_identity(&mut decoder)?;
    let captured_source_inventory = match decoder.byte()? {
        0 => None,
        1 => {
            let source_metadata_identity =
                BuildCanonicalSourceMetadataIdentity::new(decoder.u32()?, decoder.array_32()?);
            let entry_count = decoder.u64()?;
            let file_bytes = decoder.u64()?;
            if canonical_source_metadata_identity.is_none() || entry_count == 0 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "captured source inventory requires source provenance and a root entry",
                ));
            }
            Some(crate::BuildCapturedSourceInventory {
                source_metadata_identity,
                entry_count,
                file_bytes,
            })
        }
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "invalid captured source inventory tag",
            ));
        }
    };
    let replay_activation = decode_replay_activation(&mut decoder)?;
    let included_source_count = decoder.count()?;
    if included_source_count > checked_interpreter::MAX_INCLUDED_BUILD_SOURCES {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay exceeds its 256-source handoff ceiling",
        ));
    }
    let mut included_sources = Vec::new();
    included_sources
        .try_reserve_exact(included_source_count)
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source allocation failed",
            )
        })?;
    for _ in 0..included_source_count {
        included_sources.push(ShapeIncludedSource {
            relative_path: decoder.bytes()?,
            filesystem_attempt_ordinal: decoder.u64()?,
        });
    }
    let attempt_count = decoder.count()?;
    if attempt_count == 0 {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded filesystem replay record must contain at least one filesystem attempt",
        ));
    }
    let mut shapes = Vec::new();
    shapes
        .try_reserve_exact(attempt_count)
        .map_err(|_| BuildFilesystemReplayRecordError::new("replay shape allocation failed"))?;
    for _ in 0..attempt_count {
        shapes.push(decode_attempt(&mut decoder)?);
    }
    decoder.finish()?;
    validate_first_rung(&shapes)?;
    validate_included_source_shapes(&shapes, &included_sources)?;
    Ok(DecodedReplay {
        canonical_source_metadata_identity,
        captured_source_inventory,
        replay_activation,
        included_sources,
        shapes,
    })
}

pub(crate) fn decode_canonical_source_metadata_identity(
    decoder: &mut Decoder<'_>,
) -> Result<Option<BuildCanonicalSourceMetadataIdentity>, BuildFilesystemReplayRecordError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => Ok(Some(BuildCanonicalSourceMetadataIdentity::new(
            decoder.u32()?,
            decoder.array_32()?,
        ))),
        _ => Err(BuildFilesystemReplayRecordError::new(
            "invalid canonical source metadata identity tag",
        )),
    }
}
