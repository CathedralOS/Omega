//! Output file, seek, length, permission, time, sync and removal shapes.

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{
    AttemptShape, ShapeLogicalInput, ShapeLogicalInputResolution, ShapeResult, ShapeScalar,
};
use crate::evidence::replay_record::duplicates::validate_output_duplicate_shapes;
use crate::evidence::replay_record::lane_selection::{
    only_output_absent_remove_lanes, only_output_create_lanes, only_output_seek_lanes,
    only_output_set_file_permissions_lanes, only_output_set_file_times_lanes,
    only_output_set_length_lanes, only_output_sync_lanes, only_output_write_lanes,
};
use crate::evidence::replay_record::locks::validate_output_lock_shapes;
use crate::evidence::replay_record::output_ownership::validate_output_change_file_owner_shape;
use crate::evidence::replay_record::shape_validation::path_and_descriptor_shapes::validate_close_shape;

pub(crate) fn validate_output_file(
    create: &AttemptShape<'_>,
    operations: &[&AttemptShape<'_>],
    close: &AttemptShape<'_>,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let Some(output) = create.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create has no descriptor identity",
        ));
    };
    let [rooted] = create.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create has no unique rooted path",
        ));
    };
    let [authorized] = create.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create has no unique authorization",
        ));
    };
    if create.operation != 1
        || create.provider != 2
        || create.result != ShapeResult::Handle(output.identity)
        || create.post_error != 0
        || create.scalars.as_slice()
            != [(
                1,
                ShapeScalar::I32(checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
            )]
        || rooted.ordinal != 0
        || rooted.root != 1
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 1
        || authorized.root != 1
        || authorized.bytes != rooted.bytes
        || output.kind != 0
        || output.source != 0
        || output.source_identity.is_some()
        || !only_output_create_lanes(create)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output create is internally inconsistent",
        ));
    }

    let mut cursor = 0usize;
    let mut extent = 0usize;
    let mut peak_extent = 0usize;
    let mut duplicate_identities = Vec::new();
    let mut operation_cursor = 0;
    while operation_cursor < operations.len() {
        let operation = &operations[operation_cursor];
        if operation.operation == 45 {
            let close_duplicate = operations.get(operation_cursor + 1).ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate is not immediately retired",
                )
            })?;
            let duplicate_identity =
                validate_output_duplicate_shapes(operation, close_duplicate, output.identity)?;
            if duplicate_identity == output.identity
                || duplicate_identities.contains(&duplicate_identity)
            {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate identity is reused",
                ));
            }
            duplicate_identities.push(duplicate_identity);
            if duplicate_identities.len()
                > checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES
            {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output exceeds its duplicate-descriptor ceiling",
                ));
            }
            operation_cursor += 2;
            continue;
        }
        if operation.operation == 46 {
            let release = operations.get(operation_cursor + 1).ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build output lock is not immediately released",
                )
            })?;
            validate_output_lock_shapes(operation, release, output.identity)?;
            operation_cursor += 2;
            continue;
        }
        if operation.operation == 10 {
            cursor = validate_output_seek_shape(operation, output.identity, cursor, extent)?;
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 41 {
            extent = validate_output_set_length_shape(operation, output.identity)?;
            peak_extent = peak_extent.max(extent);
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 17 {
            validate_output_set_file_permissions_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 42 {
            validate_output_set_file_times_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        if matches!(operation.operation, 43 | 44) {
            validate_output_sync_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        if operation.operation == 49 {
            validate_output_change_file_owner_shape(operation, output.identity)?;
            operation_cursor += 1;
            continue;
        }
        let write = operation;
        let [(payload_ordinal, payload)] = write.byte_operands.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output write has no unique immutable payload",
            ));
        };
        let [write_input] = write.inputs.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output write has no unique descriptor input",
            ));
        };
        let payload_length = i64::try_from(payload.len()).map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "receipted build output payload exceeds this compiler host",
            )
        })?;
        let start = match write.operation {
            5 if write.scalars.is_empty() => cursor,
            7 => {
                let [(2, ShapeScalar::I64(offset))] = write.scalars.as_slice() else {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "receipted positioned output write has no unique offset",
                    ));
                };
                usize::try_from(*offset).map_err(|_| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted positioned output offset exceeds this compiler host",
                    )
                })?
            }
            _ => {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output write operation is unsupported",
                ));
            }
        };
        let end = start.checked_add(payload.len()).ok_or_else(|| {
            BuildFilesystemReplayRecordError::new("receipted build output extent overflowed")
        })?;
        if !payload.is_empty() {
            extent = extent.max(end);
            peak_extent = peak_extent.max(extent);
        }
        if write.operation == 5 {
            cursor = end;
        }
        if write.provider != 2
            || write.result != ShapeResult::Scalar(payload_length)
            || write.post_error != 0
            || *payload_ordinal != 1
            || *write_input
                != (ShapeLogicalInput {
                    ordinal: 0,
                    kind: 0,
                    resolution: ShapeLogicalInputResolution::Resolved(output.identity),
                })
            || !only_output_write_lanes(write)
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output write is internally inconsistent",
            ));
        }
        operation_cursor += 1;
    }
    if peak_extent > checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output exceeds the replay-retention ceiling",
        ));
    }
    validate_close_shape(close, output.identity)?;
    Ok(peak_extent)
}

fn validate_output_seek_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
    cursor: usize,
    extent: usize,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::I64(offset)), (2, ShapeScalar::I32(whence))] =
        operation.scalars.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output seek has no exact offset and whence",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output seek has no unique descriptor input",
        ));
    };
    let base = match whence {
        0 => 0i64,
        1 => i64::try_from(cursor).map_err(|_| {
            BuildFilesystemReplayRecordError::new("receipted build output cursor exceeds i64")
        })?,
        2 => i64::try_from(extent).map_err(|_| {
            BuildFilesystemReplayRecordError::new("receipted build output extent exceeds i64")
        })?,
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output seek whence is unsupported",
            ));
        }
    };
    let expected = base.checked_add(*offset).ok_or_else(|| {
        BuildFilesystemReplayRecordError::new("receipted build output seek result overflowed")
    })?;
    let result = usize::try_from(expected).map_err(|_| {
        BuildFilesystemReplayRecordError::new("receipted build output seek result is negative")
    })?;
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(expected)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_seek_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output seek is internally inconsistent",
        ));
    }
    Ok(result)
}

fn validate_output_set_length_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::I64(length))] = operation.scalars.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_len has no exact length",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_len has no unique descriptor input",
        ));
    };
    let length = usize::try_from(*length).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "receipted build output set_len length exceeds this compiler host",
        )
    })?;
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_set_length_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_len is internally inconsistent",
        ));
    }
    Ok(length)
}

fn validate_output_set_file_permissions_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(1, ShapeScalar::U32(_mode))] = operation.scalars.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_permissions has no exact mode",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_permissions has no unique descriptor input",
        ));
    };
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_set_file_permissions_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_permissions is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_output_set_file_times_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [(resolution_ordinal, resolution)] = operation.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times has no exact input carrier",
        ));
    };
    let [carrier] = operation.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times has no exact provider carrier",
        ));
    };
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times has no unique descriptor input",
        ));
    };
    if operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || resolution.len() < 32
        || *resolution != carrier.pre
        || carrier.pre != carrier.post
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_set_file_times_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output set_file_times is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_output_sync_shape(
    operation: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [input] = operation.inputs.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output sync has no unique descriptor input",
        ));
    };
    if !matches!(operation.operation, 43 | 44)
        || operation.provider != 2
        || operation.result != ShapeResult::Scalar(0)
        || operation.post_error != 0
        || *input
            != (ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            })
        || !only_output_sync_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output sync is internally inconsistent",
        ));
    }
    Ok(())
}

pub(crate) fn validate_output_absent_remove_shapes(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if shapes.is_empty()
        || shapes.len() > checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay absent Output removes exceed their attempt ceiling",
        ));
    }
    let mut retained_path_bytes = 0usize;
    for shape in shapes {
        let [rooted] = shape.rooted_paths.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output remove has no unique rooted path",
            ));
        };
        let [authorized] = shape.authorized_paths.as_slice() else {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output remove has no unique authorization",
            ));
        };
        retained_path_bytes = retained_path_bytes
            .checked_add(rooted.bytes.len())
            .filter(|bytes| {
                *bytes
                    <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
            })
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay absent Output remove paths exceed their aggregate ceiling",
                )
            })?;
        if !matches!(shape.operation, 9 | 12)
            || shape.provider != 2
            || shape.result != ShapeResult::Scalar(-1)
            || shape.post_error != 2
            || rooted.ordinal != 0
            || rooted.root != 1
            || !checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
            || authorized.ordinal != 0
            || authorized.access != 1
            || authorized.root != 1
            || authorized.bytes != rooted.bytes
            || !only_output_absent_remove_lanes(shape)
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay absent Output remove is internally inconsistent",
            ));
        }
    }
    Ok(())
}
