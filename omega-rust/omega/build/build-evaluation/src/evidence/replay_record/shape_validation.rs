//! Validate chronological Source/Output composition and bounded failure records.
//!
//! This file dispatches the exact record contract. `source_stream.rs` derives
//! Source lifetime membership without reordering attempts. `source_shapes.rs` validates
//! included source and source write refusal shapes, `output_stream.rs`
//! derives chronological output membership, `output_shapes.rs` validates output file
//! operations, `path_and_descriptor_shapes.rs` validates path metadata,
//! open, close, read and descriptor shapes and `native_shapes.rs`
//! validates native query, observation and handle shapes.

mod native_shapes;
mod output_shapes;
mod output_stream;
mod path_and_descriptor_shapes;
mod source_shapes;
mod source_stream;

pub(crate) use output_stream::{OutputEntryAttempts, output_tree_membership};
pub(crate) use path_and_descriptor_shapes::validate_close_shape;
pub(crate) use source_shapes::{
    validate_included_source_shapes, validate_source_write_refusal_shape,
};
pub(crate) use source_stream::{has_failure_sequence, source_output_membership};

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{AttemptShape, ShapeScalar};
use crate::evidence::replay_record::descriptor_error_state_failures::{
    unknown_descriptor_failure_with_errno_operations,
    validate_unknown_descriptor_failure_with_errno_shapes,
};
use crate::evidence::replay_record::directories::validate_output_directory_shape;
use crate::evidence::replay_record::handle_failures::{
    operand_free_unknown_descriptor_operation, unknown_descriptor_read_operation,
    unknown_descriptor_write_operation, unknown_descriptor_write_payload_operation,
    validate_operand_free_unknown_descriptor_failure_shape,
    validate_unknown_descriptor_get_osfhandle_failure_shape,
    validate_unknown_descriptor_open_at_failure_shape,
    validate_unknown_descriptor_read_failure_shape,
    validate_unknown_descriptor_read_file_metadata_failure_shape,
    validate_unknown_descriptor_seek_failure_shape,
    validate_unknown_descriptor_set_file_times_failure_shape,
    validate_unknown_descriptor_unlink_at_failure_shape,
    validate_unknown_descriptor_write_operation_failure_shape,
    validate_unknown_descriptor_write_payload_failure_shape,
    validate_unknown_native_handle_close_failure_shape,
    validate_unknown_native_handle_final_path_failure_shape,
};
use crate::evidence::replay_record::hard_links::{
    output_hard_link_paths, validate_output_hard_link_shape,
};
use crate::evidence::replay_record::native_error_state_failures::validate_unknown_native_handle_failure_with_last_error_shapes;
use crate::evidence::replay_record::native_mutation_failures::validate_unknown_native_handle_mutation_failure_shape;
use crate::evidence::replay_record::read_dir_failures::validate_unknown_descriptor_read_dir_failure_shape;
use crate::evidence::replay_record::read_links::validate_source_read_link_shape;
use crate::evidence::replay_record::shape_validation::native_shapes::{
    validate_native_error_observation_shape, validate_native_final_path_query_shape,
    validate_native_handle_close_shape, validate_native_query_open_shape,
};
use crate::evidence::replay_record::shape_validation::output_shapes::{
    validate_output_absent_remove_shapes, validate_output_file,
};
use crate::evidence::replay_record::shape_validation::path_and_descriptor_shapes::{
    validate_descriptor_metadata_shape, validate_directory_read_shape, validate_open_shape,
    validate_path_metadata_shape, validate_read_shape,
};
use crate::evidence::replay_record::symlinks::validate_output_symlink_shape;

pub(crate) fn validate_first_rung(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if has_failure_sequence(shapes) {
        return validate_failure_sequence(shapes);
    }
    let membership = source_output_membership(shapes)?;
    let output_stream = output_tree_membership(shapes, &membership.output_attempts)?;
    validate_output_entries(shapes, &output_stream)
}

fn validate_failure_sequence(
    shapes: &[AttemptShape<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    let mut cursor = 0;
    let mut identities = Vec::new();
    let mut event_count = 0;
    while cursor < shapes.len() {
        if matches!(
            shapes[cursor].operation,
            1 | 4
                | 5
                | 6
                | 7
                | 8
                | 9
                | 10
                | 11
                | 12
                | 14
                | 15
                | 17
                | 19
                | 20
                | 23
                | 27
                | 29
                | 30
                | 31
                | 32
                | 33
                | 34
                | 39
                | 41
                | 42
                | 43
                | 44
                | 45
                | 46
                | 49
        ) {
            break;
        }
        if shapes[cursor].operation == 21 {
            validate_source_read_link_shape(&shapes[cursor])?;
            cursor += 1;
            event_count += 1;
            continue;
        }
        if matches!(shapes[cursor].operation, 38 | 40) {
            validate_path_metadata_shape(&shapes[cursor])?;
            cursor += 1;
            event_count += 1;
            continue;
        }
        if shapes[cursor].operation == 28 {
            let identity = validate_native_query_open_shape(&shapes[cursor])?;
            if identities.contains(&identity) {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay source-input chains reuse a handle identity",
                ));
            }
            identities.push(identity);
            cursor += 1;
            let mut saw_final_path_query = false;
            while cursor < shapes.len() && matches!(shapes[cursor].operation, 31 | 35) {
                if shapes[cursor].operation == 31 {
                    validate_native_final_path_query_shape(&shapes[cursor], identity)?;
                    saw_final_path_query = true;
                } else {
                    validate_native_error_observation_shape(&shapes[cursor])?;
                }
                cursor += 1;
            }
            if !saw_final_path_query || cursor == shapes.len() {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Source native-handle query chain is incomplete",
                ));
            }
            validate_native_handle_close_shape(&shapes[cursor], identity)?;
            cursor += 1;
            event_count += 1;
            continue;
        }
        let identity = validate_open_shape(&shapes[cursor])?;
        if identities.contains(&identity) {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay source-read chains reuse a descriptor identity",
            ));
        }
        identities.push(identity);
        cursor += 1;

        if cursor < shapes.len() && shapes[cursor].operation == 39 {
            validate_descriptor_metadata_shape(&shapes[cursor], identity)?;
            cursor += 1;
            if cursor == shapes.len() {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay descriptor metadata chain is incomplete",
                ));
            }
            validate_close_shape(&shapes[cursor], identity)?;
            cursor += 1;
            event_count += 1;
            continue;
        }

        if cursor < shapes.len() && shapes[cursor].operation == 23 {
            let reads_start = cursor;
            while cursor < shapes.len() && shapes[cursor].operation == 23 {
                validate_directory_read_shape(&shapes[cursor], identity)?;
                cursor += 1;
            }
            if cursor == reads_start || cursor == shapes.len() {
                return Err(BuildFilesystemReplayRecordError::new(
                    "filesystem replay Source directory chain is incomplete",
                ));
            }
            validate_close_shape(&shapes[cursor], identity)?;
            cursor += 1;
            event_count += 1;
            continue;
        }

        let reads_start = cursor;
        while cursor < shapes.len() && matches!(shapes[cursor].operation, 4 | 6) {
            validate_read_shape(&shapes[cursor], identity)?;
            cursor += 1;
        }
        if cursor == reads_start || cursor == shapes.len() {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay source-read chain is incomplete",
            ));
        }
        validate_close_shape(&shapes[cursor], identity)?;
        cursor += 1;
        event_count += 1;
    }
    let begins_with_replay_suffix = cursor == 0
        && shapes.first().is_some_and(|shape| {
            matches!(
                shape.operation,
                1 | 4
                    | 5
                    | 6
                    | 7
                    | 8
                    | 9
                    | 10
                    | 11
                    | 12
                    | 14
                    | 15
                    | 17
                    | 19
                    | 20
                    | 23
                    | 27
                    | 29
                    | 30
                    | 31
                    | 32
                    | 33
                    | 34
                    | 39
                    | 41
                    | 42
                    | 43
                    | 44
                    | 45
                    | 46
                    | 49
            )
        });
    if event_count == 0 && !begins_with_replay_suffix {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay contains neither Source events nor a supported replay suffix",
        ));
    }
    if cursor < shapes.len() {
        if cursor == 0 && shapes.len() == 1 && matches!(shapes[cursor].operation, 1 | 9) {
            validate_source_write_refusal_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 2
            && unknown_descriptor_failure_with_errno_operations(&shapes[cursor..])
        {
            validate_unknown_descriptor_failure_with_errno_shapes(&shapes[cursor..])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1
            && operand_free_unknown_descriptor_operation(shapes[cursor].operation)
        {
            validate_operand_free_unknown_descriptor_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 10 {
            validate_unknown_descriptor_seek_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 14 {
            validate_unknown_descriptor_open_at_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 15 {
            validate_unknown_descriptor_unlink_at_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 23 {
            validate_unknown_descriptor_read_dir_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1
            && unknown_descriptor_write_operation(shapes[cursor].operation)
        {
            validate_unknown_descriptor_write_operation_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 42 {
            validate_unknown_descriptor_set_file_times_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && unknown_descriptor_read_operation(shapes[cursor].operation)
        {
            validate_unknown_descriptor_read_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1
            && unknown_descriptor_write_payload_operation(shapes[cursor].operation)
        {
            validate_unknown_descriptor_write_payload_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 39 {
            validate_unknown_descriptor_read_file_metadata_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 30 {
            validate_unknown_descriptor_get_osfhandle_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 29 {
            validate_unknown_native_handle_close_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && shapes[cursor].operation == 31 {
            validate_unknown_native_handle_final_path_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes.len() - cursor == 2
            && matches!(shapes[cursor].operation, 29 | 31 | 32 | 33 | 34)
            && shapes[cursor + 1].operation == 35
        {
            validate_unknown_native_handle_failure_with_last_error_shapes(&shapes[cursor..])?;
            return Ok(());
        }
        if shapes.len() - cursor == 1 && matches!(shapes[cursor].operation, 32..=34) {
            validate_unknown_native_handle_mutation_failure_shape(&shapes[cursor])?;
            return Ok(());
        }
        if shapes[cursor..]
            .iter()
            .all(|shape| matches!(shape.operation, 9 | 12))
        {
            validate_output_absent_remove_shapes(&shapes[cursor..])?;
            return Ok(());
        }
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay failure sequence contains unsupported operations",
        ));
    }
    Ok(())
}

fn validate_output_entries(
    shapes: &[AttemptShape<'_>],
    output_stream: &[OutputEntryAttempts],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if output_stream.len() > checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output exceeds the Output-tree entry ceiling",
        ));
    }
    let mut output_paths = Vec::new();
    output_paths
        .try_reserve_exact(output_stream.len())
        .map_err(|_| {
            BuildFilesystemReplayRecordError::new("filesystem replay output-path allocation failed")
        })?;
    let mut aggregate_output_extent = 0usize;
    let mut aggregate_output_duplicates = 0usize;
    let mut aggregate_output_lock_pairs = 0usize;
    let mut aggregate_path_bytes = 0usize;
    for (entry_index, range) in output_stream.iter().enumerate() {
        let path = range.path(shapes);
        if path.len() > checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output path exceeds its explicit ceiling",
            ));
        }
        aggregate_path_bytes = aggregate_path_bytes
            .checked_add(path.len())
            .filter(|bytes| {
                *bytes
                    <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
            })
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build output paths exceed their aggregate ceiling",
                )
            })?;
        if output_paths.contains(&path) {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay Output path appears more than once",
            ));
        }
        if let Some(separator) = path.iter().rposition(|byte| *byte == b'/') {
            let parent = &path[..separator];
            if !output_stream[..entry_index]
                .iter()
                .any(|prior| prior.is_directory() && prior.path(shapes) == parent)
            {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted nested Output entry does not follow its exact parent directory",
                ));
            }
        }
        output_paths.push(path);

        let attempts = match range {
            OutputEntryAttempts::Directory(index) => {
                validate_output_directory_shape(&shapes[*index])?;
                continue;
            }
            OutputEntryAttempts::HardLink(index) => {
                validate_output_hard_link_shape(&shapes[*index])?;
                let (existing, _) = output_hard_link_paths(&shapes[*index])?;
                aggregate_path_bytes = aggregate_path_bytes
                    .checked_add(existing.len())
                    .filter(|bytes| {
                        *bytes
                            <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                    })
                    .ok_or_else(|| {
                        BuildFilesystemReplayRecordError::new(
                            "receipted build output and hard-link paths exceed their aggregate ceiling",
                        )
                    })?;
                if !output_stream[..entry_index].iter().any(|prior| {
                    matches!(
                        prior,
                        OutputEntryAttempts::File { .. } | OutputEntryAttempts::HardLink(_)
                    ) && prior.path(shapes) == existing
                        && prior
                            .attempt_indices()
                            .last()
                            .is_some_and(|closed| closed < index)
                }) {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "filesystem replay Output hard link does not follow an existing regular-file name",
                    ));
                }
                continue;
            }
            OutputEntryAttempts::Symlink(index) => {
                validate_output_symlink_shape(&shapes[*index])?;
                let [(_, target)] = shapes[*index].path_like_operands.as_slice() else {
                    unreachable!("validated Output symlink has one target spelling")
                };
                aggregate_path_bytes = aggregate_path_bytes
                    .checked_add(target.len())
                    .filter(|bytes| {
                        *bytes
                            <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
                    })
                    .ok_or_else(|| {
                        BuildFilesystemReplayRecordError::new(
                            "receipted build output paths and symlink targets exceed their aggregate ceiling",
                        )
                    })?;
                continue;
            }
            OutputEntryAttempts::File { attempts } => attempts,
        };
        let chain = attempts
            .iter()
            .map(|position| &shapes[*position])
            .collect::<Vec<_>>();
        let create = &chain[0];
        let close = chain.last().expect("validated Output file has a close");
        let extent = validate_output_file(create, &chain[1..chain.len() - 1], close)?;
        aggregate_output_extent = aggregate_output_extent
            .checked_add(extent)
            .filter(|total| *total <= checked_interpreter::MAX_FILESYSTEM_REPLAY_RETAINED_BYTES)
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build outputs exceed the aggregate replay extent ceiling",
                )
            })?;
        aggregate_output_duplicates = aggregate_output_duplicates
            .checked_add(
                chain[1..chain.len() - 1]
                    .iter()
                    .filter(|operation| operation.operation == 45)
                    .count(),
            )
            .filter(|count| *count <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES)
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build outputs exceed the duplicate-descriptor ceiling",
                )
            })?;
        aggregate_output_lock_pairs = aggregate_output_lock_pairs
            .checked_add(
                chain[1..chain.len() - 1]
                    .iter()
                    .filter(|operation| {
                        operation.operation == 46
                            && operation.scalars.as_slice() == [(1, ShapeScalar::I32(6))]
                    })
                    .count(),
            )
            .filter(|count| *count <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS)
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "receipted build outputs exceed the descriptor-lock-pair ceiling",
                )
            })?;
    }
    Ok(())
}
