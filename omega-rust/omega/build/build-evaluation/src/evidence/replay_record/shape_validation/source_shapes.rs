//! Included source shapes and source write refusal shapes.

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{
    AttemptShape, ShapeRefusal, ShapeResult, ShapeScalar,
};
use crate::evidence::replay_record::rehydration::ShapeIncludedSource;
use crate::evidence::replay_record::shape_validation::output_stream::{
    OutputEntryAttempts, output_tree_membership,
};

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
    let output_stream = output_tree_membership(shapes, output_start)?;
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
        let output_index = output_stream
            .iter()
            .position(|range| {
                matches!(range, OutputEntryAttempts::File { .. })
                    && range.path(shapes) == included.relative_path
            })
            .ok_or_else(|| {
                BuildFilesystemReplayRecordError::new(
                    "filesystem replay included-source path has no matching Output file",
                )
            })?;
        let OutputEntryAttempts::File { attempts } = &output_stream[output_index] else {
            unreachable!("included source matched an Output file membership")
        };
        let earliest_ordinal = u64::try_from(attempts.last().expect("validated file closes") + 1)
            .map_err(|_| {
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
