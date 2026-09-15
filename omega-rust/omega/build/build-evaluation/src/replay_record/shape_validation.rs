//! Validating each decoded attempt shape against the replay's rungs: the
//! first rung, source refusals, output files and every handle, read, query
//! and metadata lane.

use crate::replay_record::BuildFilesystemReplayRecordError;
use crate::replay_record::attempt_codec::{
    AttemptShape, ShapeLogicalInput, ShapeLogicalInputResolution, ShapeRefusal, ShapeResult,
    ShapeScalar,
};
use crate::replay_record::descriptor_error_state_failures::{
    unknown_descriptor_failure_with_errno_operations,
    validate_unknown_descriptor_failure_with_errno_shapes,
};
use crate::replay_record::directories::validate_output_directory_shape;
use crate::replay_record::duplicates::validate_output_duplicate_shapes;
use crate::replay_record::handle_failures::{
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
use crate::replay_record::hard_links::{output_hard_link_paths, validate_output_hard_link_shape};
use crate::replay_record::lane_selection::{
    only_close_lanes, only_descriptor_metadata_lanes, only_directory_read_lanes,
    only_native_error_observation_lanes, only_native_final_path_query_lanes,
    only_native_query_open_lanes, only_open_lanes, only_output_absent_remove_lanes,
    only_output_create_lanes, only_output_seek_lanes, only_output_set_file_permissions_lanes,
    only_output_set_file_times_lanes, only_output_set_length_lanes, only_output_sync_lanes,
    only_output_write_lanes, only_path_metadata_lanes, only_read_lanes,
};
use crate::replay_record::locks::validate_output_lock_shapes;
use crate::replay_record::native_error_state_failures::validate_unknown_native_handle_failure_with_last_error_shapes;
use crate::replay_record::native_mutation_failures::validate_unknown_native_handle_mutation_failure_shape;
use crate::replay_record::output_ownership::validate_output_change_file_owner_shape;
use crate::replay_record::read_dir_failures::validate_unknown_descriptor_read_dir_failure_shape;
use crate::replay_record::read_links::validate_source_read_link_shape;
use crate::replay_record::rehydration::ShapeIncludedSource;
use crate::replay_record::symlinks::validate_output_symlink_shape;

pub(crate) fn validate_first_rung(
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
        let output_ranges = output_tree_ranges(shapes, cursor)?;
        if output_ranges.len() > checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output exceeds the Output-tree entry ceiling",
            ));
        }
        let mut output_paths = Vec::new();
        output_paths
            .try_reserve_exact(output_ranges.len())
            .map_err(|_| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay output-path allocation failed",
                )
            })?;
        let mut aggregate_output_extent = 0usize;
        let mut aggregate_output_duplicates = 0usize;
        let mut aggregate_output_lock_pairs = 0usize;
        let mut aggregate_path_bytes = 0usize;
        for (entry_index, range) in output_ranges.iter().copied().enumerate() {
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
                if !output_ranges[..entry_index]
                    .iter()
                    .any(|prior| prior.is_directory() && prior.path(shapes) == parent)
                {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "receipted nested Output entry does not follow its exact parent directory",
                    ));
                }
            }
            output_paths.push(path);

            let (start, end) = match range {
                OutputShapeRange::Directory(index) => {
                    validate_output_directory_shape(&shapes[index])?;
                    continue;
                }
                OutputShapeRange::HardLink(index) => {
                    validate_output_hard_link_shape(&shapes[index])?;
                    let (existing, _) = output_hard_link_paths(&shapes[index])?;
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
                    if !output_ranges[..entry_index].iter().any(|prior| {
                        matches!(
                            prior,
                            OutputShapeRange::File { .. } | OutputShapeRange::HardLink(_)
                        ) && prior.path(shapes) == existing
                    }) {
                        return Err(BuildFilesystemReplayRecordError::new(
                            "filesystem replay Output hard link does not follow an existing regular-file name",
                        ));
                    }
                    continue;
                }
                OutputShapeRange::Symlink(index) => {
                    validate_output_symlink_shape(&shapes[index])?;
                    let [(_, target)] = shapes[index].path_like_operands.as_slice() else {
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
                OutputShapeRange::File { start, end } => (start, end),
            };
            let chain = &shapes[start..end];
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
            let output = create
                .output
                .expect("validated output create has a descriptor");
            for identity in std::iter::once(output.identity).chain(
                chain[1..chain.len() - 1]
                    .iter()
                    .filter(|operation| operation.operation == 45)
                    .filter_map(|operation| operation.output.map(|output| output.identity)),
            ) {
                if identities.contains(&identity) {
                    return Err(BuildFilesystemReplayRecordError::new(
                        "filesystem replay Output descriptor overlaps another descriptor",
                    ));
                }
                identities.push(identity);
            }
            aggregate_output_duplicates = aggregate_output_duplicates
                .checked_add(
                    chain[1..chain.len() - 1]
                        .iter()
                        .filter(|operation| operation.operation == 45)
                        .count(),
                )
                .filter(|count| {
                    *count <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES
                })
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
                .filter(|count| {
                    *count <= checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS
                })
                .ok_or_else(|| {
                    BuildFilesystemReplayRecordError::new(
                        "receipted build outputs exceed the descriptor-lock-pair ceiling",
                    )
                })?;
        }
    }
    Ok(())
}

pub(crate) fn validate_source_write_refusal_shape(
    attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [rooted] = attempt.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay refused Source write has no unique rooted path",
        ));
    };
    let operation_is_exact = match attempt.operation {
        1 => attempt.scalars.as_slice() == [(1, ShapeScalar::I32(438))],
        9 => attempt.scalars.is_empty(),
        _ => false,
    };
    if !operation_is_exact
        || attempt.provider != 2
        || attempt.result != ShapeResult::Scalar(-1)
        || attempt.post_error != 13
        || rooted.ordinal != 0
        || rooted.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || attempt.refusals.as_slice()
            != [ShapeRefusal {
                ordinal: 0,
                access: 1,
                reason: 1,
            }]
        || !attempt.byte_operands.is_empty()
        || !attempt.path_like_operands.is_empty()
        || attempt.returned_path_count != 0
        || !attempt.returned_paths.is_empty()
        || !attempt.observed_regions.is_empty()
        || !attempt.metadata.is_empty()
        || !attempt.mutable_byte_resolutions.is_empty()
        || !attempt.mutable_i64_resolutions.is_empty()
        || !attempt.mutable_bytes.is_empty()
        || !attempt.mutable_i64s.is_empty()
        || !attempt.authorized_paths.is_empty()
        || !attempt.inputs.is_empty()
        || attempt.output.is_some()
        || !attempt.retired.is_empty()
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay refused Source write is internally inconsistent",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputShapeRange {
    Directory(usize),
    File { start: usize, end: usize },
    HardLink(usize),
    Symlink(usize),
}

impl OutputShapeRange {
    fn path<'a>(self, shapes: &'a [AttemptShape<'a>]) -> &'a [u8] {
        if let Self::HardLink(index) = self {
            return output_hard_link_paths(&shapes[index])
                .expect("validated Output hard link has exact paths")
                .1;
        }
        let index = match self {
            Self::Directory(index) | Self::File { start: index, .. } | Self::Symlink(index) => {
                index
            }
            Self::HardLink(_) => unreachable!(),
        };
        shapes[index]
            .rooted_paths
            .first()
            .expect("validated Output entry has one rooted path")
            .bytes
    }

    const fn is_directory(self) -> bool {
        matches!(self, Self::Directory(_))
    }
}

fn output_file_end(
    shapes: &[AttemptShape<'_>],
    start: usize,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    if shapes.get(start).is_none_or(|shape| shape.operation != 1) {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output file must begin with create",
        ));
    }
    let Some(root_identity) = shapes[start].output.map(|output| output.identity) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output create has no descriptor identity",
        ));
    };
    let mut cursor = start + 1;
    loop {
        if cursor == shapes.len() {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output must contain complete create-operation*-close files",
            ));
        }
        if matches!(
            shapes[cursor].operation,
            5 | 7 | 10 | 17 | 41 | 42 | 43 | 44 | 49
        ) {
            cursor += 1;
            continue;
        }
        if shapes[cursor].operation == 45 {
            if cursor + 1 >= shapes.len() || shapes[cursor + 1].operation != 8 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate must be immediately retired",
                ));
            }
            cursor += 2;
            continue;
        }
        if shapes[cursor].operation == 46 {
            if cursor + 1 >= shapes.len() || shapes[cursor + 1].operation != 46 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output lock must be immediately released",
                ));
            }
            cursor += 2;
            continue;
        }
        let closes_root = shapes[cursor].operation == 8
            && matches!(
                shapes[cursor].inputs.as_slice(),
                [ShapeLogicalInput {
                    resolution: ShapeLogicalInputResolution::Resolved(identity),
                    ..
                }] if *identity == root_identity
            );
        if closes_root {
            return Ok(cursor + 1);
        }
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output must contain complete create-operation*-close files",
        ));
    }
}

pub(crate) fn output_tree_ranges(
    shapes: &[AttemptShape<'_>],
    output_start: usize,
) -> Result<Vec<OutputShapeRange>, BuildFilesystemReplayRecordError> {
    let mut ranges = Vec::new();
    let mut cursor = output_start;
    while cursor < shapes.len() {
        match shapes[cursor].operation {
            11 => {
                ranges.push(OutputShapeRange::Directory(cursor));
                cursor += 1;
            }
            1 => {
                let end = output_file_end(shapes, cursor)?;
                ranges.push(OutputShapeRange::File { start: cursor, end });
                cursor = end;
            }
            19 | 27 => {
                ranges.push(OutputShapeRange::HardLink(cursor));
                cursor += 1;
            }
            20 => {
                ranges.push(OutputShapeRange::Symlink(cursor));
                cursor += 1;
            }
            _ => {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output must contain ordered directory, file, hard-link, or symlink entries",
                ));
            }
        }
    }
    Ok(ranges)
}

pub(crate) fn validate_included_source_shapes(
    shapes: &[AttemptShape<'_>],
    included_sources: &[ShapeIncludedSource<'_>],
) -> Result<(), BuildFilesystemReplayRecordError> {
    if included_sources.len() > checked_interpreter::MAX_INCLUDED_BUILD_SOURCES {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay exceeds its 256-source handoff ceiling",
        ));
    }
    if included_sources.is_empty() {
        return Ok(());
    }
    let output_start = shapes
        .iter()
        .position(|shape| matches!(shape.operation, 1 | 9 | 11 | 12 | 19 | 20 | 27))
        .ok_or_else(|| {
            BuildFilesystemReplayRecordError::new(
                "source-only filesystem replay cannot retain included-source handoffs",
            )
        })?;
    let output_ranges = output_tree_ranges(shapes, output_start)?;
    let total_attempt_count = u64::try_from(shapes.len()).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay attempt count exceeds canonical u64",
        )
    })?;
    let mut previous_ordinal = u64::try_from(output_start).map_err(|_| {
        BuildFilesystemReplayRecordError::new(
            "filesystem replay Source-prefix count exceeds canonical u64",
        )
    })?;
    for (handoff_index, included) in included_sources.iter().enumerate() {
        if included.filesystem_attempt_ordinal < previous_ordinal {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source ordinals are not nondecreasing",
            ));
        }
        previous_ordinal = included.filesystem_attempt_ordinal;
        if included_sources[..handoff_index]
            .iter()
            .any(|prior| prior.relative_path == included.relative_path)
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source path appears more than once",
            ));
        }
        let output_index = output_ranges
            .iter()
            .position(|range| {
                matches!(range, OutputShapeRange::File { .. })
                    && range.path(shapes) == included.relative_path
            })
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay included-source path has no matching Output file",
                )
            })?;
        let OutputShapeRange::File { end, .. } = output_ranges[output_index] else {
            unreachable!("included source matched an Output file range")
        };
        let earliest_ordinal = u64::try_from(end).map_err(|_| {
            BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source ordinal exceeds canonical u64",
            )
        })?;
        if included.filesystem_attempt_ordinal < earliest_ordinal
            || included.filesystem_attempt_ordinal > total_attempt_count
        {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay included-source handoff does not follow its Output close",
            ));
        }
    }
    Ok(())
}

fn validate_output_file(
    create: &AttemptShape<'_>,
    operations: &[AttemptShape<'_>],
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

fn validate_output_absent_remove_shapes(
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

fn validate_path_metadata_shape(
    metadata_attempt: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let expected_kind = match metadata_attempt.operation {
        38 => 0,
        40 => 2,
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "filesystem replay record has an unsupported source metadata operation",
            ));
        }
    };
    let [rooted] = metadata_attempt.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique rooted input",
        ));
    };
    let [authorized] = metadata_attempt.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique authorized target",
        ));
    };
    let [metadata] = metadata_attempt.metadata.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique semantic row",
        ));
    };
    let [(resolution_ordinal, resolution)] = metadata_attempt.mutable_byte_resolutions.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique mutable resolution",
        ));
    };
    let [carrier] = metadata_attempt.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay source metadata has no unique mutable carrier",
        ));
    };
    if metadata_attempt.provider != 2
        || metadata_attempt.result != ShapeResult::Scalar(0)
        || rooted.ordinal != 0
        || rooted.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(rooted.bytes, false)
        || authorized.ordinal != 0
        || authorized.access != 0
        || authorized.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(authorized.bytes, true)
        || metadata.ordinal != 1
        || metadata.kind != expected_kind
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post.len() < checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        || !only_path_metadata_lanes(metadata_attempt)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay source metadata is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_open_shape(open: &AttemptShape<'_>) -> Result<u64, BuildFilesystemReplayRecordError> {
    if open.operation != 2
        || open.provider != 2
        || open.scalars.as_slice() != [(1, ShapeScalar::I32(0))]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record is not a bounded source-read chain",
        ));
    }
    let Some(output) = open.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay open has no handle output",
        ));
    };
    let identity = output.identity;
    let [open_rooted] = open.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay open has no unique rooted source path",
        ));
    };
    let [open_authorized] = open.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay open has no unique authorized source path",
        ));
    };
    if output.kind != 0
        || output.source != 0
        || output.source_identity.is_some()
        || open.result != ShapeResult::Handle(identity)
        || open_rooted.ordinal != 0
        || open_rooted.root != 0
        || open_authorized.ordinal != 0
        || open_authorized.access != 0
        || open_authorized.root != 0
        || open_authorized.bytes != open_rooted.bytes
        || !only_open_lanes(open)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent descriptor creation",
        ));
    }
    Ok(identity)
}

pub(crate) fn validate_close_shape(
    close: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if close.operation != 8
        || close.provider != 2
        || close.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || close.result != ShapeResult::Scalar(0)
        || close.retired.as_slice() != [identity]
        || !only_close_lanes(close)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent descriptor retirement",
        ));
    }
    Ok(())
}

fn validate_read_shape(
    read: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if read.provider != 2
        || read.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay read has inconsistent descriptor lineage",
        ));
    }
    let ShapeResult::Scalar(read_result) = read.result else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has a non-scalar result",
        ));
    };
    let Ok(read_length) = u64::try_from(read_result) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read did not succeed",
        ));
    };
    let (requested, expected_region_kind) = match (read.operation, read.scalars.as_slice()) {
        (4, [(2, ShapeScalar::U64(requested))]) => (requested, 0),
        (
            6,
            [
                (2, ShapeScalar::U64(requested)),
                (3, ShapeScalar::I64(offset)),
            ],
        ) if *offset >= 0 => (requested, 1),
        _ => {
            return Err(BuildFilesystemReplayRecordError::new(
                "bounded replay read has no exact transfer count and positioned offset",
            ));
        }
    };
    let [region] = read.observed_regions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has no unique observed region",
        ));
    };
    let [(resolution_ordinal, resolution)] = read.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has no unique mutable resolution",
        ));
    };
    let [carrier] = read.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read has no unique mutable carrier",
        ));
    };
    let Ok(read_end) = usize::try_from(read_length) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay read length exceeds this host",
        ));
    };
    if region.ordinal != 1
        || region.kind != expected_region_kind
        || region.offset != 0
        || region.length != read_length
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || read_end > carrier.post.len()
        || read_length > *requested
        || u64::try_from(carrier.post.len()).is_ok_and(|capacity| *requested > capacity)
        || carrier.pre[read_end..] != carrier.post[read_end..]
        || !only_read_lanes(read)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay read carrier is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_directory_read_shape(
    read: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let ShapeResult::Scalar(result) = read.result else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has a non-scalar result",
        ));
    };
    let Ok(result_length) = u64::try_from(result) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay did not succeed",
        ));
    };
    let [(2, ShapeScalar::U64(requested))] = read.scalars.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no exact transfer count",
        ));
    };
    let [region] = read.observed_regions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique observed region",
        ));
    };
    let [(resolution_ordinal, resolution)] = read.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique byte resolution",
        ));
    };
    let [carrier] = read.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique byte carrier",
        ));
    };
    let [(position_resolution_ordinal, _)] = read.mutable_i64_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique cursor resolution",
        ));
    };
    let [position] = read.mutable_i64s.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay has no unique cursor carrier",
        ));
    };
    let Ok(result_end) = usize::try_from(result_length) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded directory replay length exceeds this host",
        ));
    };
    if read.operation != 23
        || read.provider != 2
        || read.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || region.ordinal != 1
        || region.kind != 2
        || region.offset != 0
        || region.length != result_length
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || resolution.len() != carrier.pre.len()
        || carrier.pre.len() != carrier.post.len()
        || result_end > carrier.post.len()
        || result_length > *requested
        || u64::try_from(carrier.post.len()).is_ok_and(|capacity| *requested > capacity)
        || carrier.pre[result_end..] != carrier.post[result_end..]
        || *position_resolution_ordinal != 3
        || position.ordinal != 3
        || !only_directory_read_lanes(read)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay directory carrier is internally inconsistent",
        ));
    }
    Ok(())
}

fn validate_descriptor_metadata_shape(
    metadata_attempt: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let [metadata] = metadata_attempt.metadata.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay descriptor metadata has no unique semantic row",
        ));
    };
    let [(resolution_ordinal, resolution)] = metadata_attempt.mutable_byte_resolutions.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay descriptor metadata has no unique mutable resolution",
        ));
    };
    let [carrier] = metadata_attempt.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay descriptor metadata has no unique mutable carrier",
        ));
    };
    if metadata_attempt.operation != 39
        || metadata_attempt.provider != 2
        || metadata_attempt.result != ShapeResult::Scalar(0)
        || metadata_attempt.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 0,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || metadata.ordinal != 1
        || metadata.kind != 1
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post.len() < checked_interpreter::FILESYSTEM_METADATA_API_CARRIER_BYTES
        || !only_descriptor_metadata_lanes(metadata_attempt)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay descriptor metadata is internally inconsistent",
        ));
    }
    Ok(())
}

/// Identity acquired by one constrained Source `open_path_handle` (tag 28)
/// under the bounded query-only contract. The scalar row commits to access
/// zero, full read/write/delete sharing, null security attributes,
/// `OPEN_EXISTING`, and `FILE_FLAG_BACKUP_SEMANTICS` without
/// `FILE_FLAG_DELETE_ON_CLOSE`; the single handle input is the null template.
fn validate_native_query_open_shape(
    open: &AttemptShape<'_>,
) -> Result<u64, BuildFilesystemReplayRecordError> {
    if open.operation != 28
        || open.provider != 2
        || open.scalars.as_slice()
            != [
                (1, ShapeScalar::U32(0)),
                (2, ShapeScalar::U32(0x7)),
                (3, ShapeScalar::I64(0)),
                (4, ShapeScalar::U32(3)),
                (5, ShapeScalar::U32(0x0200_0000)),
            ]
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record is not a bounded native-handle query chain",
        ));
    }
    let Some(output) = open.output else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle open has no handle output",
        ));
    };
    let identity = output.identity;
    let [open_rooted] = open.rooted_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle open has no unique rooted source path",
        ));
    };
    let [open_authorized] = open.authorized_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle open has no unique authorized source path",
        ));
    };
    if output.kind != 1
        || output.source != 0
        || output.source_identity.is_some()
        || open.result != ShapeResult::Handle(identity)
        || open_rooted.ordinal != 0
        || open_rooted.root != 0
        || !checked_interpreter::filesystem_root_relative_path_is_canonical(
            open_rooted.bytes,
            false,
        )
        || open_authorized.ordinal != 0
        || open_authorized.access != 0
        || open_authorized.root != 0
        || open_authorized.bytes != open_rooted.bytes
        || open.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 6,
                kind: 1,
                resolution: ShapeLogicalInputResolution::Null,
            }]
        || !only_native_query_open_lanes(open)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent native-handle acquisition",
        ));
    }
    Ok(identity)
}

/// One `final_path_name_by_handle` (tag 31) observation on `identity` inside
/// a bounded query-release chain. The buffer custody arithmetic mirrors the
/// checked-interpreter record contract: the resolved snapshot precedes the
/// call, the post state carries the returned path plus its NUL terminator
/// over an unchanged tail, and the scalar result is the returned path
/// length.
fn validate_native_final_path_query_shape(
    query: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    let ShapeResult::Scalar(result) = query.result else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has a non-scalar result",
        ));
    };
    let [
        (2, ShapeScalar::U64(capacity)),
        (3, ShapeScalar::U32(_flags)),
    ] = query.scalars.as_slice()
    else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no exact capacity and flags",
        ));
    };
    let [returned] = query.returned_paths.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no unique returned path",
        ));
    };
    let [(resolution_ordinal, resolution)] = query.mutable_byte_resolutions.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no unique mutable resolution",
        ));
    };
    let [carrier] = query.mutable_bytes.as_slice() else {
        return Err(BuildFilesystemReplayRecordError::new(
            "bounded replay native-handle final-path query has no unique mutable carrier",
        ));
    };
    let path_length = returned.bytes.len();
    let capacity_fits = usize::try_from(*capacity)
        .is_ok_and(|capacity| capacity <= carrier.post.len() && path_length < capacity);
    if query.operation != 31
        || query.provider != 2
        || path_length == 0
        || i64::try_from(path_length) != Ok(result)
        || !capacity_fits
        || returned.ordinal != 1
        || returned.kind != 2
        || returned.completeness != 0
        || *resolution_ordinal != 1
        || carrier.ordinal != 1
        || *resolution != carrier.pre
        || carrier.pre.len() != carrier.post.len()
        || carrier.post[..path_length] != returned.bytes[..]
        || carrier.post[path_length] != 0
        || carrier.post[path_length + 1..] != carrier.pre[path_length + 1..]
        || query.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 1,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || !only_native_final_path_query_lanes(query)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle final-path query is internally inconsistent",
        ));
    }
    Ok(())
}

/// One handle-free `get_last_error` (tag 35) error-slot read inside a
/// bounded query-release chain. The read observes the slot without clearing
/// it, so the scalar result equals the recorded post-error.
fn validate_native_error_observation_shape(
    operation: &AttemptShape<'_>,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if operation.operation != 35
        || operation.provider != 2
        || operation.result != ShapeResult::Scalar(i64::from(operation.post_error))
        || !only_native_error_observation_lanes(operation)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay native-handle error observation is internally inconsistent",
        ));
    }
    Ok(())
}

/// The successful `close_handle` (tag 29) that retires `identity` at the end
/// of a bounded query-release chain.
fn validate_native_handle_close_shape(
    close: &AttemptShape<'_>,
    identity: u64,
) -> Result<(), BuildFilesystemReplayRecordError> {
    if close.operation != 29
        || close.provider != 2
        || close.inputs.as_slice()
            != [ShapeLogicalInput {
                ordinal: 0,
                kind: 1,
                resolution: ShapeLogicalInputResolution::Resolved(identity),
            }]
        || !matches!(close.result, ShapeResult::Scalar(result) if result != 0)
        || close.retired.as_slice() != [identity]
        || !only_close_lanes(close)
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay record has inconsistent native-handle retirement",
        ));
    }
    Ok(())
}
